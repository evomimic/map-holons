use core_types::ContentSet;
use holons_loader_client::BOOTSTRAP_IMPORT_SCHEMA_PATH;
use holons_prelude::prelude::*;
use std::path::PathBuf;

use super::{map_core_schema_paths, read_file_data, CoreSchemaLoadMetrics};

const DOMAIN_SCHEMA_RELATIVE_PATH: &str =
    "import_files/MAP Schema Types-map-test-schema-book-person-inverse.json";

pub const CORE_AND_BOOK_PERSON_INVERSE_METRICS: CoreSchemaLoadMetrics = CoreSchemaLoadMetrics {
    staged: 0,
    committed: 0,
    links_created: 0,
    errors: 0,
    total_bundles: 8,
    total_loader_holons: 189,
};

pub fn domain_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DOMAIN_SCHEMA_RELATIVE_PATH)
}

pub fn build_core_and_book_person_inverse_content_set() -> Result<ContentSet, HolonError> {
    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_SCHEMA_PATH);
    let schema = read_file_data(&schema_path, "validation schema")?;

    let files_to_load = map_core_schema_paths()
        .into_iter()
        .map(|path| read_file_data(&path, "core schema import"))
        .chain(std::iter::once(read_file_data(
            &domain_schema_path(),
            "Book/Person inverse test schema import",
        )))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ContentSet { schema, files_to_load })
}
