use holons_test::{TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

// TODO: need to match on expected content

/// This function builds and dances a `query_relationships` DanceRequest for the supplied source TestReference and QueryExpression.
pub async fn execute_query_relationships(
    state: &mut TestExecutionState,
    source_token: TestReference,
    query_expression: QueryExpression,
    expected_status: ResponseStatusCode,
    description:Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Querying Relationships".to_string()
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(&context,
 &source_token).unwrap();

    let node_collection =
        NodeCollection { members: vec![Node::new(source_reference, None)], query_spec: None };

    // 2. BUILD - the query_relationships DanceRequest
    let request = build_query_relationships_dance_request(node_collection, query_expression)
        .expect("Failed to build query_relationships request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request)
.await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "query_relationships request returned unexpected status: {}",
        response.description
    );

    // TODO:  Match on response.body node collection expected vs actual
}
