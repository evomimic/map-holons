//! Loads configured schema imports into the local HolonSpace.
//!
//! Imports are ordinary loader inputs. They may contribute Dancers, Themes,
//! or other Holons and their authored relationships. Conductora does not
//! select, activate, or otherwise special-case any imported Dancer.

use base_types::ToBaseValue;
use core_types::{ContentSet, FileData};
use holons_core::ReadableHolon;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::ExecutionPolicy;
use serde::Deserialize;
use std::path::{Component, Path};
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;

const IMPORT_ROOT: &str = "resources";
const IMPORT_SET_FILENAME: &str = "space-imports.json";

#[derive(Debug, Deserialize)]
struct SpaceImportSet {
    imports: Vec<String>,
}

/// Loads every configured schema package through the ordinary post-Core loader.
/// Runtime Dancer discovery is exclusively relationship-driven after commit.
pub async fn ensure_configured_space_imports(handle: &AppHandle) -> anyhow::Result<()> {
    let resource_root = handle
        .path()
        .resource_dir()
        .map_err(|error| anyhow::anyhow!("resolving Conductora resource directory: {error}"))?;
    let import_root = resource_root.join(IMPORT_ROOT);
    let import_set: SpaceImportSet = serde_json::from_slice(
        &std::fs::read(import_root.join(IMPORT_SET_FILENAME))
            .map_err(|error| anyhow::anyhow!("reading configured space imports: {error}"))?,
    )
    .map_err(|error| anyhow::anyhow!("parsing configured space imports: {error}"))?;

    let runtime = handle
        .try_state::<RuntimeState>()
        .ok_or_else(|| anyhow::anyhow!("RuntimeState is not managed"))?
        .read()
        .map_err(|error| anyhow::anyhow!("reading RuntimeState: {error}"))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MAP Commands runtime is not initialized"))?;

    for import in import_set.imports {
        let started = std::time::Instant::now();
        let import = safe_relative_path(&import)?;
        let path = import_root.join(import);
        let content_set = ContentSet {
            files_to_load: vec![FileData {
                filename: import.to_string_lossy().into_owned(),
                raw_contents: std::fs::read_to_string(&path).map_err(|error| {
                    anyhow::anyhow!("reading schema import {}: {error}", path.display())
                })?,
            }],
        };
        // Core bootstrap provisions a local HolonSpace for this launch. A
        // schema package may target that space through its ordinary authored
        // relationships, so an existing package holon cannot make this load
        // safely skippable on a later launch.
        let tx_id = runtime.session().begin_transaction().await?;
        let context = runtime.session().get_transaction(&tx_id)?;
        let result = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context,
                    action: TransactionAction::LoadHolons { content_set },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .and_then(|result| {
                let MapResult::Reference(response) = result else {
                    return Err(core_types::HolonError::InvalidState(
                        "schema import returned no load response".into(),
                    ));
                };
                let status = response.property_value("LoadCommitStatus")?;
                if status != Some("Complete".to_base_value()) {
                    return Err(core_types::HolonError::InvalidState(format!(
                        "schema import {} did not complete: {status:?}",
                        import.display()
                    )));
                }
                Ok(())
            });
        runtime.session().archive_transaction(&tx_id)?;
        if std::env::var_os("MAP_PROFILE").is_some() {
            eprintln!(
                "[MAP-PROFILE] import={} total_ms={:.3} succeeded={}",
                import.display(),
                started.elapsed().as_secs_f64() * 1000.0,
                result.is_ok()
            );
        }
        result?;
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> anyhow::Result<&Path> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!("space import path must be relative: {value}");
    }
    Ok(path)
}
