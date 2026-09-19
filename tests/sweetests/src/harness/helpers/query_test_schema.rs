use core_types::ContentSet;
use holons_prelude::prelude::*;
use std::path::PathBuf;

use super::read_file_data;

/// Sweettest-only Query test schema: a concrete `UnimplementedQueryExpression`
/// extending `QueryExpression.HolonType`. Compiled from
/// `schema-src/test/query-unimplemented-expression.tdl`; never part of the
/// production bootstrap bundle.
const GENERATED_QUERY_TEST_SCHEMA_FILENAME: &str = "test/query-unimplemented-expression.json";

/// Descriptor key of the test-only expression type.
pub const UNIMPLEMENTED_QUERY_EXPRESSION_DESCRIPTOR_KEY: &str =
    "UnimplementedQueryExpression.HolonType";

pub fn query_test_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("generated/json-imports")
        .join(GENERATED_QUERY_TEST_SCHEMA_FILENAME)
}

pub fn build_query_test_schema_content_set() -> Result<ContentSet, HolonError> {
    let files_to_load = vec![read_file_data(
        &query_test_schema_path(),
        "Query unimplemented-expression test schema import",
    )?];

    Ok(ContentSet { files_to_load })
}
