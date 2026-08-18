use holons_prelude::prelude::*;

use holons_test::harness::helpers::{
    build_core_schema_content_set, build_generated_core_schema_content_set,
    build_generated_query_dance_schema_content_set, build_generated_query_schema_content_set,
    build_generated_validation_schema_content_set, CORE_SCHEMA_METRICS,
    GENERATED_CORE_SCHEMA_METRICS,
};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::{
    execute_load_holons_client, execute_load_holons_client_expect_success,
};

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
        CORE_SCHEMA_METRICS.commit_status,
    )
    .await;
}

/// Loads the validation-owned generated package after Core has committed.
pub async fn execute_load_generated_validation_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_validation_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated validation schema ContentSet: {error:?}")
    });

    execute_load_holons_client_expect_success(test_state, content_set, 155, 1).await;
}

/// Loads the independently usable Query package after Core has committed.
pub async fn execute_load_generated_query_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_query_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Query Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set, 43, 1).await;
}

/// Loads the Dance-layer Query adapter after Core and Query have committed.
pub async fn execute_load_generated_query_dance_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_query_dance_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Query Dance Adapter Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set, 13, 1).await;
}

/// Loads the checked-in Schema 2.0 compiler artifact. Metrics are recorded by
/// the acceptance fixture once the first successful import establishes them.
pub async fn execute_load_generated_core_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_core_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Schema 2.0 core ContentSet: {error:?}")
    });

    execute_load_holons_client(
        test_state,
        content_set,
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.staged),
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.committed),
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.links_created),
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.errors),
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.total_bundles),
        MapInteger(GENERATED_CORE_SCHEMA_METRICS.total_loader_holons),
        GENERATED_CORE_SCHEMA_METRICS.commit_status,
    )
    .await;
}
