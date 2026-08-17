use crate::ExpectedLoadStatus;
use core_types::{ContentSet, FileData};
use holons_loader_client::BOOTSTRAP_IMPORT_VALIDATION_SCHEMA_PATH;
use holons_prelude::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

const GENERATED_CORE_SCHEMA_FILENAMES: [&str; 12] = [
    "MAP Schema Types-map-core-schema-abstract-value-types.json",
    "MAP Schema Types-map-core-schema-command-types.json",
    "MAP Schema Types-map-core-schema-concrete-value-types.json",
    "MAP Schema Types-map-core-schema-dance-schema.json",
    "MAP Schema Types-map-core-schema-keyrules-schema.json",
    "MAP Schema Types-map-core-schema-loader-types.json",
    "MAP Schema Types-map-core-schema-operator-types.json",
    "MAP Schema Types-map-core-schema-property-types.json",
    "MAP Schema Types-map-core-schema-query-schema.json",
    "MAP Schema Types-map-core-schema-relationship-types.json",
    "MAP Schema Types-map-core-schema-root.json",
    "MAP Schema Types-map-core-schema-value-constraint-types.json",
];

const GENERATED_VALIDATION_SCHEMA_FILENAME: &str = "map-validation-schema.json";

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
    staged: 376,
    committed: 376,
    links_created: 1740,
    errors: 0,
    total_bundles: 12,
    total_loader_holons: 376,
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
    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_VALIDATION_SCHEMA_PATH);
    let schema = read_file_data(&schema_path, "validation schema")?;
    let files_to_load = map_core_schema_paths()
        .into_iter()
        .map(|path| read_file_data(&path, "core schema import"))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ContentSet { schema, files_to_load })
}

/// Builds the Schema 2.0 generated-artifact regression input. Compilation is
/// deliberately outside this test: it consumes the checked-in JSON artifact.
pub fn build_generated_core_schema_content_set() -> Result<ContentSet, HolonError> {
    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_VALIDATION_SCHEMA_PATH);
    let schema = read_file_data(&schema_path, "validation schema")?;
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

    Ok(ContentSet { schema, files_to_load })
}

/// Builds the checked-in validation package artifact. Callers must have
/// loaded the generated Core schema in an earlier completed transaction.
pub fn build_generated_validation_schema_content_set() -> Result<ContentSet, HolonError> {
    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_VALIDATION_SCHEMA_PATH);
    let schema = read_file_data(&schema_path, "validation schema")?;
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let validation_schema_path =
        repo_root.join("generated/json-imports").join(GENERATED_VALIDATION_SCHEMA_FILENAME);
    let files_to_load =
        vec![read_file_data(&validation_schema_path, "generated validation schema import")?];

    Ok(ContentSet { schema, files_to_load })
}

pub fn read_file_data(path: &Path, role: &str) -> Result<FileData, HolonError> {
    let raw_contents = fs::read_to_string(path).map_err(|error| {
        HolonError::Misc(format!("failed to read {role} file {}: {error}", path.display()))
    })?;

    Ok(FileData { filename: path.display().to_string(), raw_contents })
}
