use std::{
    collections::BTreeSet,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use mr_bundle::{Bundle, FileSystemBundler, ResourceBytes};
use serde::{Deserialize, Serialize};
use wasmparser::{ExternalKind, Payload};

/// Verifies the composition and exact export surface of MAP hApp artifacts.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Coordinator-surface policy and exact export inventory.
    #[arg(long)]
    manifest: PathBuf,

    /// Packed production DNA to inspect.
    #[arg(long)]
    dna: Option<PathBuf>,

    /// Packed production hApp whose embedded DNAs will be inspected.
    #[arg(long)]
    happ: Option<PathBuf>,

    /// Loose test-probe coordinator WASM to inspect.
    #[arg(long)]
    probe_wasm: Option<PathBuf>,

    /// Reject test-only symbols in production artifacts.
    ///
    /// Phase 2 leaves this switch off while the probes still exist in `holons`. Phase 6 enables
    /// it after the test-only externs have moved out of the production coordinator.
    #[arg(long)]
    deny_production_test_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceManifest {
    production_coordinator_zomes: BTreeSet<String>,
    #[serde(rename = "export")]
    exports: Vec<ClassifiedExport>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassifiedExport {
    zome: String,
    symbol: String,
    exposure_kind: ExposureKind,
    classification: Classification,
    rationale: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ExposureKind {
    ZomeCall,
    Callback,
    IntegrityCallback,
    Abi,
    Global,
    Memory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Classification {
    Supported,
    LegacyIngress,
    Callback,
    Abi,
    TestOnly,
}

/// Minimal DNA manifest model for artifact inspection.
///
/// The audit needs only zome names and bundled resource identifiers. Keeping this transport model
/// local avoids pulling Holochain's native runtime dependency tree into a deterministic build tool.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct DnaManifest {
    manifest_version: String,
    name: String,
    integrity: IntegrityManifest,
    #[serde(default)]
    coordinator: CoordinatorManifest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct IntegrityManifest {
    #[serde(default)]
    network_seed: Option<String>,
    #[serde(default)]
    properties: Option<serde_yaml::Value>,
    zomes: Vec<ZomeManifest>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct CoordinatorManifest {
    #[serde(default)]
    zomes: Vec<ZomeManifest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ZomeManifest {
    name: String,
    #[serde(default)]
    hash: Option<String>,
    path: String,
    #[serde(default)]
    dependencies: Option<Vec<ZomeDependency>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ZomeDependency {
    name: String,
}

/// Minimal hApp manifest model used to locate every embedded DNA resource.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct HappManifest {
    manifest_version: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    roles: Vec<HappRole>,
    #[serde(default)]
    allow_deferred_memproofs: bool,
    #[serde(default)]
    bootstrap_url: Option<String>,
    #[serde(default)]
    relay_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HappRole {
    name: String,
    #[serde(default)]
    provisioning: Option<serde_yaml::Value>,
    dna: HappRoleDna,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HappRoleDna {
    path: Option<String>,
    #[serde(default)]
    modifiers: serde_yaml::Value,
    #[serde(default)]
    installed_hash: Option<String>,
    #[serde(default)]
    clone_limit: u32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WasmExport {
    symbol: String,
    kind: WasmExportKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum WasmExportKind {
    Function,
    Table,
    Memory,
    Global,
    Tag,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.dna.is_none() && cli.happ.is_none() && cli.probe_wasm.is_none() {
        bail!("provide at least one artifact: --dna, --happ, or --probe-wasm");
    }

    let policy = read_surface_manifest(&cli.manifest)?;
    validate_surface_manifest(&policy)?;

    if let Some(path) = &cli.dna {
        let bundle = FileSystemBundler::load_from::<DnaManifest>(path)
            .await
            .with_context(|| format!("failed to load DNA bundle {}", path.display()))?;
        audit_production_dna(&bundle, path.display(), &policy, cli.deny_production_test_only)?;
    }

    if let Some(path) = &cli.happ {
        audit_happ(path, &policy, cli.deny_production_test_only).await?;
    }

    if let Some(path) = &cli.probe_wasm {
        audit_probe_wasm(path, &policy)?;
    }

    println!("hApp artifact audit passed");
    Ok(())
}

fn read_surface_manifest(path: &Path) -> Result<SurfaceManifest> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read surface manifest {}", path.display()))?;
    toml::from_str(&source)
        .with_context(|| format!("failed to parse surface manifest {}", path.display()))
}

fn validate_surface_manifest(policy: &SurfaceManifest) -> Result<()> {
    if policy.production_coordinator_zomes.is_empty() {
        bail!("surface manifest must approve at least one production coordinator zome");
    }

    let mut seen = BTreeSet::new();
    for export in &policy.exports {
        if export.rationale.trim().is_empty() {
            bail!("{}::{} has an empty rationale", export.zome, export.symbol);
        }
        if !seen.insert((&export.zome, &export.symbol)) {
            bail!("duplicate classification for {}::{}", export.zome, export.symbol);
        }
        validate_classification_pair(export)?;
    }

    for zome in &policy.production_coordinator_zomes {
        if !policy.exports.iter().any(|export| &export.zome == zome) {
            bail!("approved production coordinator {zome} has no classified exports");
        }
    }

    Ok(())
}

fn validate_classification_pair(export: &ClassifiedExport) -> Result<()> {
    let valid = match export.exposure_kind {
        ExposureKind::ZomeCall => matches!(
            export.classification,
            Classification::Supported | Classification::LegacyIngress | Classification::TestOnly
        ),
        ExposureKind::Callback | ExposureKind::IntegrityCallback => {
            export.classification == Classification::Callback
        }
        ExposureKind::Abi | ExposureKind::Global | ExposureKind::Memory => {
            export.classification == Classification::Abi
        }
    };
    if !valid {
        bail!(
            "{}::{} has incompatible exposure kind {:?} and classification {:?}",
            export.zome,
            export.symbol,
            export.exposure_kind,
            export.classification
        );
    }
    Ok(())
}

async fn audit_happ(
    path: &Path,
    policy: &SurfaceManifest,
    deny_production_test_only: bool,
) -> Result<()> {
    let bundle = FileSystemBundler::load_from::<HappManifest>(path)
        .await
        .with_context(|| format!("failed to load hApp bundle {}", path.display()))?;

    let mut audited_dnas = 0;
    for role in &bundle.manifest().roles {
        let Some(resource_id) = role.dna.path.as_deref() else {
            // A role pinned only by installed hash has no packaged DNA to inspect.
            continue;
        };
        let dna_bytes = required_resource(
            &bundle,
            resource_id,
            &format!("hApp role {} in {}", role.name, path.display()),
        )?;
        let dna_bundle = Bundle::<DnaManifest>::unpack(Cursor::new(dna_bytes.as_ref()))
            .with_context(|| {
                format!("failed to unpack DNA resource {resource_id} for hApp role {}", role.name)
            })?;
        let label = format!("{} role {} DNA {resource_id}", path.display(), role.name);
        audit_production_dna(&dna_bundle, &label, policy, deny_production_test_only)?;
        audited_dnas += 1;
    }

    if audited_dnas == 0 {
        bail!("hApp {} contains no embedded DNA resources", path.display());
    }
    Ok(())
}

fn audit_production_dna(
    bundle: &Bundle<DnaManifest>,
    label: impl std::fmt::Display,
    policy: &SurfaceManifest,
    deny_production_test_only: bool,
) -> Result<()> {
    let label = label.to_string();
    let actual_zomes: BTreeSet<_> =
        bundle.manifest().coordinator.zomes.iter().map(|zome| zome.name.clone()).collect();

    if actual_zomes != policy.production_coordinator_zomes {
        bail!(
            "coordinator composition mismatch in {label}: expected {}, found {}",
            display_set(&policy.production_coordinator_zomes),
            display_set(&actual_zomes)
        );
    }

    for zome in &bundle.manifest().coordinator.zomes {
        let wasm = required_resource(bundle, &zome.path, &label)?;
        audit_exact_exports(
            &zome.name,
            wasm.as_ref(),
            &format!("{label} coordinator {}", zome.name),
            policy,
        )?;

        if deny_production_test_only {
            // This manifest lookup is authoritative only because the exact inventory check above
            // has already proved that the manifest and packaged WASM are equivalent for this zome.
            let test_only: BTreeSet<_> = policy
                .exports
                .iter()
                .filter(|export| {
                    export.zome == zome.name && export.classification == Classification::TestOnly
                })
                .map(|export| export.symbol.as_str())
                .collect();
            if !test_only.is_empty() {
                bail!(
                    "production coordinator {} contains test-only exports: {}",
                    zome.name,
                    display_set(&test_only)
                );
            }
        }
    }

    Ok(())
}

fn audit_probe_wasm(path: &Path, policy: &SurfaceManifest) -> Result<()> {
    let probe_zomes: BTreeSet<_> = policy
        .exports
        .iter()
        .filter(|export| export.classification == Classification::TestOnly)
        .filter(|export| !policy.production_coordinator_zomes.contains(&export.zome))
        .map(|export| export.zome.as_str())
        .collect();
    if probe_zomes.len() != 1 {
        bail!(
            "probe audit requires exactly one non-production zome with test-only exports; found {}",
            display_set(&probe_zomes)
        );
    }
    let probe_zome = probe_zomes.iter().next().expect("length checked above");
    let wasm =
        fs::read(path).with_context(|| format!("failed to read probe WASM {}", path.display()))?;
    audit_exact_exports(probe_zome, &wasm, &path.display().to_string(), policy)?;

    let actual_symbols: BTreeSet<_> =
        parse_wasm_exports(&wasm)?.into_iter().map(|export| export.symbol).collect();
    let production_calls: BTreeSet<_> = policy
        .exports
        .iter()
        .filter(|export| {
            policy.production_coordinator_zomes.contains(&export.zome)
                && export.exposure_kind == ExposureKind::ZomeCall
                && export.classification != Classification::TestOnly
                && actual_symbols.contains(&export.symbol)
        })
        .map(|export| export.symbol.as_str())
        .collect();
    if !production_calls.is_empty() {
        bail!(
            "probe WASM unexpectedly exports production zome-call functions: {}",
            display_set(&production_calls)
        );
    }
    Ok(())
}

fn audit_exact_exports(
    zome: &str,
    wasm: &[u8],
    label: &str,
    policy: &SurfaceManifest,
) -> Result<()> {
    let actual = parse_wasm_exports(wasm)
        .with_context(|| format!("failed to parse exports from {label}"))?;
    let expected: BTreeSet<_> = policy
        .exports
        .iter()
        .filter(|export| export.zome == zome)
        .map(|export| WasmExport {
            symbol: export.symbol.clone(),
            kind: expected_wasm_kind(export.exposure_kind),
        })
        .collect();

    let missing: BTreeSet<_> = expected.difference(&actual).cloned().collect();
    let unexpected: BTreeSet<_> = actual.difference(&expected).cloned().collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        bail!(
            "export inventory mismatch for {label}: missing [{}]; unexpected [{}]",
            display_exports(&missing),
            display_exports(&unexpected)
        );
    }
    Ok(())
}

fn parse_wasm_exports(wasm: &[u8]) -> Result<BTreeSet<WasmExport>> {
    let mut exports = BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Payload::ExportSection(reader) = payload? {
            for export in reader {
                let export = export?;
                let kind = match export.kind {
                    ExternalKind::Func => WasmExportKind::Function,
                    ExternalKind::Table => WasmExportKind::Table,
                    ExternalKind::Memory => WasmExportKind::Memory,
                    ExternalKind::Global => WasmExportKind::Global,
                    ExternalKind::Tag => WasmExportKind::Tag,
                };
                exports.insert(WasmExport { symbol: export.name.to_string(), kind });
            }
        }
    }
    Ok(exports)
}

fn expected_wasm_kind(exposure_kind: ExposureKind) -> WasmExportKind {
    match exposure_kind {
        ExposureKind::Memory => WasmExportKind::Memory,
        ExposureKind::Global => WasmExportKind::Global,
        ExposureKind::ZomeCall
        | ExposureKind::Callback
        | ExposureKind::IntegrityCallback
        | ExposureKind::Abi => WasmExportKind::Function,
    }
}

fn required_resource<'a, M>(
    bundle: &'a Bundle<M>,
    resource_id: &str,
    context: &str,
) -> Result<&'a ResourceBytes>
where
    M: std::fmt::Debug + Serialize + serde::de::DeserializeOwned,
{
    bundle.get_resource(&resource_id.to_string()).ok_or_else(|| {
        anyhow::anyhow!("resource {resource_id} referenced by {context} is absent from the bundle")
    })
}

fn display_set<T: std::fmt::Display>(values: &BTreeSet<T>) -> String {
    values.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

fn display_exports(exports: &BTreeSet<WasmExport>) -> String {
    exports
        .iter()
        .map(|export| format!("{} ({:?})", export.symbol, export.kind))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_pairs_reject_application_roles_on_generated_exports() {
        let export = ClassifiedExport {
            zome: "holons".to_string(),
            symbol: "memory".to_string(),
            exposure_kind: ExposureKind::Memory,
            classification: Classification::Supported,
            rationale: "invalid pairing".to_string(),
        };

        assert!(validate_classification_pair(&export).is_err());
    }

    #[test]
    fn exact_inventory_detects_export_kind_changes() {
        // A complete minimal module containing one no-argument function exported as `f`.
        let wasm = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // magic and version
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section
            0x03, 0x02, 0x01, 0x00, // function section
            0x07, 0x05, 0x01, 0x01, b'f', 0x00, 0x00, // export section
            0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // code section
        ];
        let mut policy = SurfaceManifest {
            production_coordinator_zomes: BTreeSet::from(["holons".to_string()]),
            exports: vec![ClassifiedExport {
                zome: "holons".to_string(),
                symbol: "f".to_string(),
                exposure_kind: ExposureKind::ZomeCall,
                classification: Classification::Supported,
                rationale: "Test function.".to_string(),
            }],
        };

        audit_exact_exports("holons", &wasm, "minimal test WASM", &policy)
            .expect("the function export should match its classification");

        policy.exports[0].exposure_kind = ExposureKind::Global;
        policy.exports[0].classification = Classification::Abi;
        let error = audit_exact_exports("holons", &wasm, "minimal test WASM", &policy)
            .expect_err("a function exported where a global is classified must fail");

        assert!(error.to_string().contains("export inventory mismatch"));
        assert!(error.to_string().contains("f (Global)"));
        assert!(error.to_string().contains("f (Function)"));
    }
}
