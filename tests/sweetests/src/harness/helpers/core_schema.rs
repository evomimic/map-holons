use crate::ExpectedLoadStatus;
use core_types::{ContentSet, FileData};
use holons_prelude::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
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

const GENERATED_DANCE_SCHEMA_FILENAME: &str = "dance/schema.json";
const GENERATED_COMMANDS_SCHEMA_FILENAME: &str = "commands/schema.json";
const GENERATED_VALIDATION_SCHEMA_FILENAME: &str = "validation/schema.json";
const GENERATED_QUERY_SCHEMA_FILENAME: &str = "query/schema.json";
const GENERATED_QUERY_DANCE_SCHEMA_FILENAME: &str = "query-dance/schema.json";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CoreSchemaLoadMetrics {
    pub staged: i64,
    pub committed: i64,
    pub links_created: i64,
    pub errors: i64,
    pub total_bundles: i64,
    pub total_loader_holons: i64,
    pub commit_status: ExpectedLoadStatus,
}

/// Metrics for the checked-in Schema 2.0 compiler artifact.
pub const GENERATED_CORE_SCHEMA_METRICS: CoreSchemaLoadMetrics = CoreSchemaLoadMetrics {
    staged: 279,
    committed: 279,
    links_created: 0,
    errors: 0,
    total_bundles: 10,
    total_loader_holons: 279,
    commit_status: ExpectedLoadStatus::Complete,
};

/// The standard Sweettest Core bootstrap uses the checked-in Schema 2.0
/// compiler artifact. The legacy host import corpus is no longer a supported
/// runtime compatibility path.
pub const CORE_SCHEMA_METRICS: CoreSchemaLoadMetrics = GENERATED_CORE_SCHEMA_METRICS;

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
