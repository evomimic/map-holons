use holons_prelude::prelude::*;

use holons_test::harness::helpers::{build_core_schema_content_set, CORE_SCHEMA_METRICS};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::execute_load_holons_client;

pub async fn execute_load_core_schema(test_state: &mut TestExecutionState) {
    let content_set = build_core_schema_content_set()
        .unwrap_or_else(|error| panic!("failed to build MAP core schema ContentSet: {error:?}"));

    execute_load_holons_client(
        test_state,
        content_set,
        MapInteger(CORE_SCHEMA_METRICS.staged),
        MapInteger(CORE_SCHEMA_METRICS.committed),
        MapInteger(CORE_SCHEMA_METRICS.links_created),
        MapInteger(CORE_SCHEMA_METRICS.errors),
        MapInteger(CORE_SCHEMA_METRICS.total_bundles),
        MapInteger(CORE_SCHEMA_METRICS.total_loader_holons),
    )
    .await;
}
