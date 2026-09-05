//! Conductora-owned lifecycle state for first-space Core Schema provisioning.
//!
//! The loader job itself is deliberately internal to Conductora.  Normal IPC
//! ingress may observe this gate but cannot select bootstrap provisioning.

use core_types::{ContentSet, FileData, HolonError};
use holons_core::reference_layer::HolonSpaceBehavior;
use map_commands_contract::{MapCommand, TransactionAction, TransactionCommand};
use map_commands_runtime::ExecutionPolicy;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path};
use std::sync::RwLock;
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;

// Tauri preserves the source `resources/` path below the runtime resource
// directory (including in dev mode under `target/debug/resources`).
const BUNDLE_DIRECTORY: &str = "resources/core-schema-bootstrap";
const MANIFEST_FILENAME: &str = "manifest.json";
const REQUIRED_BOOTSTRAP_PACKAGES: [&str; 7] =
    ["core", "validation", "dance", "commands", "query", "query-dance", "dahn"];

#[derive(Debug, Deserialize)]
struct BootstrapManifest {
    release_identity: String,
    core_schema_key: String,
    core_schema_space_key: String,
    packages: Vec<BootstrapPackage>,
    imports: Vec<BootstrapImport>,
}

#[derive(Debug, Deserialize)]
struct BootstrapPackage {
    name: String,
    import_directory: String,
}

#[derive(Debug, Deserialize)]
struct BootstrapImport {
    path: String,
    sha256: String,
}

/// The only process-local states in which Conductora may accept MAP work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSchemaBootstrapPhase {
    /// Startup provisioning is running; normal command ingress is blocked.
    Bootstrapping,
    /// The CoreSchemaSpace was committed, verified, and injected into runtime.
    Ready,
    /// Provisioning failed; normal command ingress remains blocked.
    Failed,
}

/// Process-local gate that protects normal ingress until first-space
/// provisioning has completed.
#[derive(Debug)]
pub struct CoreSchemaBootstrapGate {
    phase: RwLock<CoreSchemaBootstrapPhase>,
}

impl Default for CoreSchemaBootstrapGate {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreSchemaBootstrapGate {
    /// Creates a closed gate for application startup.
    pub fn new() -> Self {
        Self { phase: RwLock::new(CoreSchemaBootstrapPhase::Bootstrapping) }
    }

    /// Returns the current bootstrap phase.
    pub fn phase(&self) -> Result<CoreSchemaBootstrapPhase, HolonError> {
        self.phase.read().map(|phase| *phase).map_err(|error| {
            HolonError::FailedToAcquireLock(format!(
                "CoreSchemaBootstrapGate lock poisoned: {error}"
            ))
        })
    }

    /// Opens normal ingress only after bootstrap readiness is verified.
    pub fn mark_ready(&self) -> Result<(), HolonError> {
        self.set_phase(CoreSchemaBootstrapPhase::Ready)
    }

    /// Retains a failed state so a running process cannot accept normal work
    /// after bootstrap has failed.
    pub fn mark_failed(&self) -> Result<(), HolonError> {
        self.set_phase(CoreSchemaBootstrapPhase::Failed)
    }

    /// Rejects normal ingress until provisioning has verified readiness.
    pub fn ensure_ready(&self) -> Result<(), HolonError> {
        match self.phase()? {
            CoreSchemaBootstrapPhase::Ready => Ok(()),
            CoreSchemaBootstrapPhase::Bootstrapping => Err(HolonError::ServiceNotAvailable(
                "Core Schema bootstrap is still in progress".to_string(),
            )),
            CoreSchemaBootstrapPhase::Failed => Err(HolonError::ServiceNotAvailable(
                "Core Schema bootstrap failed; restart after resolving the provisioning error"
                    .to_string(),
            )),
        }
    }

    fn set_phase(&self, phase: CoreSchemaBootstrapPhase) -> Result<(), HolonError> {
        let mut current = self.phase.write().map_err(|error| {
            HolonError::FailedToAcquireLock(format!(
                "CoreSchemaBootstrapGate lock poisoned: {error}"
            ))
        })?;
        *current = phase;
        Ok(())
    }
}

/// Reads the packaged, manifest-selected bootstrap inputs into the existing
/// loader `ContentSet` transport shape. The caller still owns the bootstrap
/// transaction and must not expose this content through ordinary IPC.
pub fn packaged_bootstrap_content_set(handle: &AppHandle) -> anyhow::Result<ContentSet> {
    let resource_root = handle
        .path()
        .resource_dir()
        .map_err(|error| anyhow::anyhow!("resolving Conductora resource directory: {error}"))?;
    bootstrap_content_set_from_directory(&resource_root.join(BUNDLE_DIRECTORY))
}

/// Ensures the shared runtime has a local CoreSchemaSpace before normal
/// Conductora ingress is opened. This is intentionally called directly from
/// startup, never through a Tauri command.
pub async fn ensure_core_schema_space(handle: &AppHandle) -> anyhow::Result<()> {
    let gate = handle
        .try_state::<CoreSchemaBootstrapGate>()
        .ok_or_else(|| anyhow::anyhow!("CoreSchemaBootstrapGate is not managed"))?;
    let runtime_state = handle
        .try_state::<RuntimeState>()
        .ok_or_else(|| anyhow::anyhow!("RuntimeState is not managed"))?;
    let runtime = runtime_state
        .read()
        .map_err(|error| anyhow::anyhow!("reading RuntimeState: {error}"))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MAP Commands runtime is not initialized"))?;

    if runtime.session().space_manager().get_space_holon_id()?.is_some() {
        gate.mark_ready()?;
        return Ok(());
    }

    let content_set = packaged_bootstrap_content_set(handle)?;
    let tx_id = runtime.session().begin_transaction().await?;
    let context = runtime.session().get_transaction(&tx_id)?;
    context.enable_bootstrap_provisioning();

    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::LoadHolons { content_set },
            }),
            ExecutionPolicy::default(),
        )
        .await;
    runtime.session().archive_transaction(&tx_id)?;
    result?;

    if runtime.session().space_manager().get_space_holon_id()?.is_none() {
        return Err(anyhow::anyhow!(
            "Core Schema bootstrap completed without injecting a LocalHolonSpace"
        ));
    }

    gate.mark_ready()?;
    Ok(())
}

fn bootstrap_content_set_from_directory(bundle_directory: &Path) -> anyhow::Result<ContentSet> {
    let manifest_path = bundle_directory.join(MANIFEST_FILENAME);
    let manifest: BootstrapManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .map_err(|error| anyhow::anyhow!("reading {}: {error}", manifest_path.display()))?,
    )
    .map_err(|error| anyhow::anyhow!("parsing {}: {error}", manifest_path.display()))?;

    if manifest.release_identity.is_empty()
        || manifest.core_schema_key.is_empty()
        || manifest.core_schema_space_key != "MAP.CoreSchemaSpace"
        || manifest.imports.is_empty()
        || manifest.packages.len() != REQUIRED_BOOTSTRAP_PACKAGES.len()
        || manifest
            .packages
            .iter()
            .map(|package| package.name.as_str())
            .ne(REQUIRED_BOOTSTRAP_PACKAGES)
        || manifest.packages.iter().any(|package| package.import_directory != package.name)
    {
        anyhow::bail!("Core Schema bootstrap manifest is incomplete or incompatible");
    }

    let mut files_to_load = Vec::with_capacity(manifest.imports.len());
    for import in manifest.imports {
        let relative_path = Path::new(&import.path);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            anyhow::bail!("bootstrap manifest contains unsafe import path {}", import.path);
        }

        let path = bundle_directory.join(relative_path);
        let raw_contents = std::fs::read_to_string(&path)
            .map_err(|error| anyhow::anyhow!("reading {}: {error}", path.display()))?;
        let actual_digest = sha256_hex(raw_contents.as_bytes());
        if actual_digest != import.sha256 {
            anyhow::bail!(
                "bootstrap import digest mismatch for {} (expected {}, found {})",
                import.path,
                import.sha256,
                actual_digest
            );
        }

        files_to_load.push(FileData { filename: import.path, raw_contents });
    }

    Ok(ContentSet { files_to_load })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn gate_rejects_ingress_until_marked_ready() {
        let gate = CoreSchemaBootstrapGate::new();

        assert!(gate.ensure_ready().is_err());
        gate.mark_ready().expect("mark ready");
        assert!(gate.ensure_ready().is_ok());
    }

    #[test]
    fn failed_gate_remains_closed() {
        let gate = CoreSchemaBootstrapGate::new();

        gate.mark_failed().expect("mark failed");
        assert_eq!(gate.phase().expect("phase"), CoreSchemaBootstrapPhase::Failed);
        assert!(gate.ensure_ready().is_err());
    }

    #[test]
    fn bundle_reader_accepts_manifest_selected_imports() -> anyhow::Result<()> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = std::env::temp_dir().join(format!("conductora-bootstrap-{suffix}"));
        let imports = directory.join("imports");
        fs::create_dir_all(&imports)?;
        let contents = "{\"holons\":[]}";
        let digest = sha256_hex(contents.as_bytes());
        fs::write(imports.join("core.json"), contents)?;
        fs::write(
            directory.join(MANIFEST_FILENAME),
            format!(
                "{{\"release_identity\":\"MAP Core Schema-v0.0.7\",\"core_schema_key\":\"MAP Core Schema-v0.0.7\",\"core_schema_space_key\":\"MAP.CoreSchemaSpace\",\"packages\":[{{\"name\":\"core\",\"import_directory\":\"core\"}},{{\"name\":\"validation\",\"import_directory\":\"validation\"}},{{\"name\":\"dance\",\"import_directory\":\"dance\"}},{{\"name\":\"commands\",\"import_directory\":\"commands\"}},{{\"name\":\"query\",\"import_directory\":\"query\"}},{{\"name\":\"query-dance\",\"import_directory\":\"query-dance\"}},{{\"name\":\"dahn\",\"import_directory\":\"dahn\"}}],\"imports\":[{{\"path\":\"imports/core.json\",\"sha256\":\"{digest}\"}}]}}"
            ),
        )?;

        let content_set = bootstrap_content_set_from_directory(&directory)?;
        assert_eq!(content_set.files_to_load.len(), 1);
        assert_eq!(content_set.files_to_load[0].filename, "imports/core.json");

        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn bundle_reader_rejects_digest_mismatch() -> anyhow::Result<()> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = std::env::temp_dir().join(format!("conductora-bootstrap-{suffix}"));
        let imports = directory.join("imports");
        fs::create_dir_all(&imports)?;
        fs::write(imports.join("core.json"), "{\"holons\":[]}")?;
        fs::write(
            directory.join(MANIFEST_FILENAME),
            "{\"release_identity\":\"MAP Core Schema-v0.0.7\",\"core_schema_key\":\"MAP Core Schema-v0.0.7\",\"core_schema_space_key\":\"MAP.CoreSchemaSpace\",\"packages\":[{\"name\":\"core\",\"import_directory\":\"core\"},{\"name\":\"validation\",\"import_directory\":\"validation\"},{\"name\":\"dance\",\"import_directory\":\"dance\"},{\"name\":\"commands\",\"import_directory\":\"commands\"},{\"name\":\"query\",\"import_directory\":\"query\"},{\"name\":\"query-dance\",\"import_directory\":\"query-dance\"},{\"name\":\"dahn\",\"import_directory\":\"dahn\"}],\"imports\":[{\"path\":\"imports/core.json\",\"sha256\":\"invalid\"}]}",
        )?;

        assert!(bootstrap_content_set_from_directory(&directory).is_err());
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn packaged_bundle_matches_the_generated_bootstrap_bundle() -> anyhow::Result<()> {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let generated_bundle = repository_root.join("generated/core-schema-bootstrap");
        let packaged_bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join(BUNDLE_DIRECTORY);

        assert_eq!(
            fs::read(generated_bundle.join(MANIFEST_FILENAME))?,
            fs::read(packaged_bundle.join(MANIFEST_FILENAME))?,
            "Conductora's packaged bootstrap manifest must be refreshed from the generated bundle"
        );
        assert_eq!(
            bootstrap_content_set_from_directory(&generated_bundle)?.files_to_load,
            bootstrap_content_set_from_directory(&packaged_bundle)?.files_to_load,
            "Conductora's packaged bootstrap imports must match the generated bundle"
        );

        Ok(())
    }
}
