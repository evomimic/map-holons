use holons_test::harness::helpers::{
    assert_descriptor_completion, build_core_schema_content_set,
    build_generated_commands_schema_content_set, build_generated_core_schema_content_set,
    build_generated_dance_schema_content_set, build_generated_query_dance_schema_content_set,
    build_generated_query_schema_content_set, build_generated_validation_schema_content_set,
    expected_descriptor_keys,
};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::execute_load_holons_client_expect_success;

pub async fn execute_load_core_schema(test_state: &mut TestExecutionState) {
    let content_set = build_core_schema_content_set()
        .unwrap_or_else(|error| panic!("failed to build MAP core schema ContentSet: {error:?}"));

    let descriptor_keys = expected_descriptor_keys(&content_set);
    execute_load_holons_client_expect_success(test_state, content_set).await;
    assert_descriptor_completion(&test_state.context(), descriptor_keys);
}

/// Loads the validation-owned generated package after Core has committed.
pub async fn execute_load_generated_validation_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_validation_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated validation schema ContentSet: {error:?}")
    });

    let descriptor_keys = expected_descriptor_keys(&content_set);
    execute_load_holons_client_expect_success(test_state, content_set).await;
    assert_descriptor_completion(&test_state.context(), descriptor_keys);
}

/// Loads the Dance package after Core has committed.
pub async fn execute_load_generated_dance_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_dance_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Dance Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set).await;
}

/// Loads the Commands package after Core and Dance have committed.
pub async fn execute_load_generated_commands_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_commands_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Commands Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set).await;
}

/// Loads the independently usable Query package after Core has committed.
pub async fn execute_load_generated_query_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_query_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Query Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set).await;
}

/// Loads the Dance-layer Query adapter after Core and Query have committed.
pub async fn execute_load_generated_query_dance_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_query_dance_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Query Dance Adapter Schema ContentSet: {error:?}")
    });
    execute_load_holons_client_expect_success(test_state, content_set).await;
}

/// Loads the checked-in Schema 2.0 compiler artifact.
pub async fn execute_load_generated_core_schema(test_state: &mut TestExecutionState) {
    let content_set = build_generated_core_schema_content_set().unwrap_or_else(|error| {
        panic!("failed to build generated Schema 2.0 core ContentSet: {error:?}")
    });

    let descriptor_keys = expected_descriptor_keys(&content_set);
    execute_load_holons_client_expect_success(test_state, content_set).await;
    assert_descriptor_completion(&test_state.context(), descriptor_keys);
}
