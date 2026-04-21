use core_types::ContentSet;
use holons_loader_client::BOOTSTRAP_IMPORT_SCHEMA_PATH;
use holons_prelude::prelude::*;
use std::path::PathBuf;

use super::{read_file_data, CoreSchemaLoadMetrics};

const DOMAIN_SCHEMA_RELATIVE_PATH: &str =
    "import_files/MAP Schema Types-map-test-schema-book-person-inverse.json";

pub const BOOK_PERSON_INVERSE_METRICS: CoreSchemaLoadMetrics = CoreSchemaLoadMetrics {
    staged: 7,
    committed: 7,
    links_created: 39,
    errors: 0,
    total_bundles: 1,
    total_loader_holons: 7,
};

pub fn domain_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DOMAIN_SCHEMA_RELATIVE_PATH)
}

pub fn build_book_person_inverse_content_set() -> Result<ContentSet, HolonError> {
    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_SCHEMA_PATH);
    let schema = read_file_data(&schema_path, "validation schema")?;

    let files_to_load =
        vec![read_file_data(&domain_schema_path(), "Book/Person inverse test schema import")?];

    Ok(ContentSet { schema, files_to_load })
}
