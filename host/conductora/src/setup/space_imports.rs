//! Loads configured schema imports into the local HolonSpace.
//!
//! Imports are ordinary loader inputs. They may contribute Dancers, Themes,
//! or other Holons and their authored relationships. Conductora does not
//! select, activate, or otherwise special-case any imported Dancer.

use core_types::{ContentSet, FileData};
use map_commands_contract::{MapCommand, TransactionAction, TransactionCommand};
use map_commands_runtime::ExecutionPolicy;
use serde::Deserialize;
use std::path::{Component, Path};
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;

const IMPORT_ROOT: &str = "resources/house-troupe";
const IMPORT_SET_FILENAME: &str = "space-imports.json";
const PACKAGE_MANIFEST_FILENAME: &str = "package.json";

#[derive(Debug, Deserialize)]
struct SpaceImportSet {
    imports: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SchemaImportManifest {
    schema_imports: Vec<String>,
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

    for package_directory in import_set.imports {
        let started = std::time::Instant::now();
        let package_directory = safe_relative_path(&package_directory)?;
        let package_root = import_root.join(package_directory);
        let manifest: SchemaImportManifest = serde_json::from_slice(
            &std::fs::read(package_root.join(PACKAGE_MANIFEST_FILENAME))
                .map_err(|error| anyhow::anyhow!("reading schema import manifest: {error}"))?,
        )
        .map_err(|error| anyhow::anyhow!("parsing schema import manifest: {error}"))?;

        let content_set = ContentSet {
            files_to_load: manifest
                .schema_imports
                .iter()
                .map(|import| {
                    let import = safe_relative_path(import)?;
                    let path = package_root.join(import);
                    Ok(FileData {
                        filename: path
                            .file_name()
                            .ok_or_else(|| anyhow::anyhow!("schema import has no file name"))?
                            .to_string_lossy()
                            .into_owned(),
                        raw_contents: std::fs::read_to_string(&path).map_err(|error| {
                            anyhow::anyhow!("reading schema import {}: {error}", path.display())
                        })?,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
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
            .await;
        runtime.session().archive_transaction(&tx_id)?;
        if std::env::var_os("MAP_PROFILE").is_some() {
            eprintln!(
                "[MAP-PROFILE] package={} total_ms={:.3} succeeded={}",
                package_directory.display(),
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
