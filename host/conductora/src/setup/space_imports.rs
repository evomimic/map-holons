//! Loads configured definition packages only on first provisioning and checks
//! the fixed packaged dependencies on Open. It never activates a Dancer.
use core_types::{ContentSet, FileData};
use holons_core::ReadableHolon;
use map_commands_contract::{MapCommand, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use serde::Deserialize;
use std::path::{Component, Path};
use tauri::{AppHandle, Manager};

use super::core_schema_bootstrap::{require_complete_load, CoreSchemaStartup};
use crate::runtime::RuntimeState;

const IMPORT_ROOT: &str = "resources";
const IMPORT_SET_FILENAME: &str = "space-imports.json";

#[derive(Debug, Deserialize)]
struct SpaceImportSet {
    imports: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DefinitionFile {
    holons: Vec<RequiredDefinition>,
}
#[derive(Debug, Deserialize)]
struct RequiredDefinition {
    key: String,
    #[serde(rename = "type")]
    descriptor: String,
    #[serde(default)]
    relationships: Vec<RequiredRelationship>,
}
#[derive(Debug, Deserialize)]
struct RequiredRelationship {
    name: String,
    target: Vec<RequiredTarget>,
}
#[derive(Debug, Deserialize)]
struct RequiredTarget {
    #[serde(rename = "$ref")]
    key: String,
}

pub async fn ensure_configured_space_imports(
    handle: &AppHandle,
    startup: CoreSchemaStartup,
) -> anyhow::Result<()> {
    let resources = handle.path().resource_dir()?;
    let runtime = handle
        .state::<RuntimeState>()
        .read()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MAP Commands runtime is not initialized"))?;
    prepare_space_imports(&runtime, &resources, startup).await
}

/// The same orchestration runs in the packaged host and real-runtime tests.
/// Existing state is inspected, never repaired by replaying create imports.
pub(crate) async fn prepare_space_imports(
    runtime: &Runtime,
    resources: &Path,
    startup: CoreSchemaStartup,
) -> anyhow::Result<()> {
    let import_root = resources.join(IMPORT_ROOT);
    let imports: SpaceImportSet =
        serde_json::from_slice(&std::fs::read(import_root.join(IMPORT_SET_FILENAME))?)?;
    let mut required = Vec::new();
    for import in imports.imports {
        let relative = safe_relative_path(&import)?;
        let raw = std::fs::read_to_string(import_root.join(relative))?;
        let definitions: DefinitionFile = serde_json::from_str(&raw)?;
        required.extend(definitions.holons);
        if startup == CoreSchemaStartup::OpenedExisting {
            continue;
        }
        let started = std::time::Instant::now();
        let tx = runtime.session().begin_transaction().await?;
        let context = runtime.session().get_transaction(&tx)?;
        let result = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context,
                    action: TransactionAction::LoadHolons {
                        content_set: ContentSet {
                            files_to_load: vec![FileData {
                                filename: import.clone(),
                                raw_contents: raw,
                            }],
                        },
                    },
                }),
                ExecutionPolicy::default(),
            )
            .await?;
        require_complete_load(&result, &format!("schema import {import}"))?;
        runtime.session().archive_transaction(&tx)?;
        tracing::info!(
            "Provisioned configured definitions: {import}, elapsed_ms={}",
            started.elapsed().as_millis()
        );
    }
    verify_definitions(runtime, &required).await
}

async fn verify_definitions(
    runtime: &Runtime,
    required: &[RequiredDefinition],
) -> anyhow::Result<()> {
    let tx = runtime.session().begin_transaction().await?;
    let checked = (|| -> anyhow::Result<()> {
        let context = runtime.session().get_transaction(&tx)?;
        for definition in required {
            let saved = context.lookup().get_saved_holon_by_key(&definition.key.clone().into())?;
            let relationships =
                std::iter::once(("DescribedBy", vec![definition.descriptor.as_str()])).chain(
                    definition.relationships.iter().map(|r| {
                        (r.name.as_str(), r.target.iter().map(|t| t.key.as_str()).collect())
                    }),
                );
            for (name, expected) in relationships {
                let related = saved.related_holons(name)?;
                let targets: Vec<_> = related
                    .read()
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                    .clone()
                    .into_iter()
                    .collect();
                let keys = targets.iter().map(|r| r.key()).collect::<Result<Vec<_>, _>>()?;
                for key in expected {
                    if !keys.iter().flatten().any(|actual| actual.0 == key) {
                        anyhow::bail!("Packaged foundation is incomplete or incompatible: {} lacks {name} -> {key}; explicit remediation/Upgrade is required", definition.key);
                    }
                }
            }
        }
        Ok(())
    })();
    let archived = runtime.session().archive_transaction(&tx);
    checked?;
    archived?;
    tracing::info!(
        "Verified packaged definition dependencies without import replay: {} definitions",
        required.len()
    );
    Ok(())
}

fn safe_relative_path(value: &str) -> anyhow::Result<&Path> {
    let path = Path::new(value);
    if path.is_absolute() || path.components().any(|c| !matches!(c, Component::Normal(_))) {
        anyhow::bail!("space import path must be a normal relative path: {value}");
    }
    Ok(path)
}
