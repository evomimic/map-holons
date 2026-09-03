use core_types::{ContentSet, FileData};
use holons_prelude::prelude::*;
use serde::Deserialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const GENERATED_CORE_SCHEMA_FILENAMES: [&str; 10] = [
    "core/abstract-value-types.json",
    "core/concrete-value-types.json",
    "core/keyrules.json",
    "core/loader-types.json",
    "core/operator-types.json",
    "core/property-types.json",
    "core/relationship-types.json",
    "core/root.json",
    "core/value-constraint-types.json",
    "core/validation.json",
];

#[derive(Debug, Deserialize)]
struct CoreSchemaBootstrapManifest {
    imports: Vec<CoreSchemaBootstrapImport>,
}

#[derive(Debug, Deserialize)]
struct CoreSchemaBootstrapImport {
    path: String,
}

const GENERATED_DANCE_SCHEMA_FILENAME: &str = "dance/schema.json";
const GENERATED_COMMANDS_SCHEMA_FILENAME: &str = "commands/schema.json";
const GENERATED_VALIDATION_SCHEMA_FILENAME: &str = "validation/schema.json";
const GENERATED_QUERY_SCHEMA_FILENAME: &str = "query/schema.json";
const GENERATED_QUERY_DANCE_SCHEMA_FILENAME: &str = "query-dance/schema.json";

/// Absolute paths to all core schema import files used for loader-client testing.
pub fn map_core_schema_paths() -> Vec<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");

    GENERATED_CORE_SCHEMA_FILENAMES
        .iter()
        .map(|filename| repo_root.join("generated/json-imports").join(filename))
        .collect()
}

pub fn build_core_schema_content_set() -> Result<ContentSet, HolonError> {
    let files_to_load = map_core_schema_paths()
        .into_iter()
        .map(|path| read_file_data(&path, "core schema import"))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ContentSet { files_to_load })
}

/// Builds the generated first-space bundle used by the Conductora bootstrap
/// service. Sweettests use this exact graph so ordinary test transactions
/// never depend on the removed lazy LocalHolonSpace creation path.
pub fn build_core_schema_bootstrap_content_set() -> Result<ContentSet, HolonError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let bundle_directory = repo_root.join("generated/core-schema-bootstrap");
    let manifest_path = bundle_directory.join("manifest.json");
    let manifest: CoreSchemaBootstrapManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
            HolonError::Misc(format!(
                "failed to read bootstrap manifest {}: {error}",
                manifest_path.display()
            ))
        })?)
        .map_err(|error| {
            HolonError::Misc(format!(
                "failed to parse bootstrap manifest {}: {error}",
                manifest_path.display()
            ))
        })?;

    if manifest.imports.is_empty() {
        return Err(HolonError::Misc("Core Schema bootstrap manifest has no imports".to_string()));
    }

    let files_to_load = manifest
        .imports
        .iter()
        .map(|import| {
            let relative_path = Path::new(&import.path);
            if relative_path.is_absolute()
                || relative_path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(HolonError::Misc(format!(
                    "Core Schema bootstrap manifest contains unsafe import path {}",
                    import.path
                )));
            }
            read_file_data(&bundle_directory.join(relative_path), "Core Schema bootstrap import")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ContentSet { files_to_load })
}

/// Builds the Schema 2.0 generated-artifact regression input. Compilation is
/// deliberately outside this test: it consumes the checked-in JSON artifact.
pub fn build_generated_core_schema_content_set() -> Result<ContentSet, HolonError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let files_to_load = GENERATED_CORE_SCHEMA_FILENAMES
        .iter()
        .map(|filename| {
            read_file_data(
                &repo_root.join("generated/json-imports").join(filename),
                "generated core schema import",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ContentSet { files_to_load })
}

/// Builds the checked-in validation package artifact. Callers must have
/// loaded the generated Core schema in an earlier completed transaction.
pub fn build_generated_validation_schema_content_set() -> Result<ContentSet, HolonError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let validation_schema_path =
        repo_root.join("generated/json-imports").join(GENERATED_VALIDATION_SCHEMA_FILENAME);
    let files_to_load =
        vec![read_file_data(&validation_schema_path, "generated validation schema import")?];

    Ok(ContentSet { files_to_load })
}

/// Builds the Dance package after Core has committed.
pub fn build_generated_dance_schema_content_set() -> Result<ContentSet, HolonError> {
    build_generated_schema_package_content_set(
        GENERATED_DANCE_SCHEMA_FILENAME,
        "generated dance schema import",
    )
}

/// Builds the Commands package after Core and Dance have committed.
pub fn build_generated_commands_schema_content_set() -> Result<ContentSet, HolonError> {
    build_generated_schema_package_content_set(
        GENERATED_COMMANDS_SCHEMA_FILENAME,
        "generated commands schema import",
    )
}

/// Builds the independently loadable Query Schema package after Core has committed.
pub fn build_generated_query_schema_content_set() -> Result<ContentSet, HolonError> {
    build_generated_schema_package_content_set(
        GENERATED_QUERY_SCHEMA_FILENAME,
        "generated query schema import",
    )
}

/// Builds the Query--Dance adapter package after Core, Dance, and Query have committed.
pub fn build_generated_query_dance_schema_content_set() -> Result<ContentSet, HolonError> {
    build_generated_schema_package_content_set(
        GENERATED_QUERY_DANCE_SCHEMA_FILENAME,
        "generated query dance adapter schema import",
    )
}

fn build_generated_schema_package_content_set(
    filename: &str,
    role: &str,
) -> Result<ContentSet, HolonError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let schema_path = repo_root.join("generated/json-imports").join(filename);
    Ok(ContentSet { files_to_load: vec![read_file_data(&schema_path, role)?] })
}

pub fn read_file_data(path: &Path, role: &str) -> Result<FileData, HolonError> {
    let raw_contents = fs::read_to_string(path).map_err(|error| {
        HolonError::Misc(format!("failed to read {role} file {}: {error}", path.display()))
    })?;

    Ok(FileData { filename: path.display().to_string(), raw_contents })
}
