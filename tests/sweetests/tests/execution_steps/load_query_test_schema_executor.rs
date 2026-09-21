use holons_test::harness::helpers::build_query_test_schema_content_set;
use holons_test::TestExecutionState;

use super::load_holons_client_executor::execute_load_holons_client_expect_success;

/// Loads the Sweettest-only Query test schema (`UnimplementedQueryExpression`)
/// on top of the bootstrapped Core/Query/QueryDance schemas.
pub async fn execute_load_query_test_schema(test_state: &mut TestExecutionState) {
    let content_set = build_query_test_schema_content_set()
        .unwrap_or_else(|error| panic!("failed to build Query test schema ContentSet: {error:?}"));

    execute_load_holons_client_expect_success(test_state, content_set).await;
}
