use crate::mock_conductor::MockConductorConfig;
use async_std::task;
use holons_prelude::prelude::*;
use pretty_assertions::assert_eq;
use rstest::*;
use std::collections::BTreeMap;
use tracing::{debug, info};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::*;
use crate::shared_test::{
    // mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};

use holons_core::core_shared_objects::ReadableHolonState; // TODO: Eliminate this dependency

/// This function builds and dances a `query_relationships` DanceRequest for the supplied NodeCollection and QueryExpression.
pub async fn execute_query_relationships(
    test_state: &mut DanceTestExecutionState,
    source_key: MapString,
    query_expression: QueryExpression,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Querying Relationships ---");

    // 1. Retrieve the source Holon
    let source_holon = test_state
        .get_created_holon_by_key(&source_key)
        .unwrap_or_else(|| panic!("Holon with key {:?} not found in created_holons", source_key));

    let source_holon_id = source_holon
        .holon_id()
        .expect(&format!("Failed to get local_id for Holon: {:#?}", source_holon));

    let holon_reference = HolonReference::Smart(SmartReference::new_from_id(source_holon_id));

    let node_collection =
        NodeCollection { members: vec![Node::new(holon_reference, None)], query_spec: None };

    // 2. Build the query_relationships DanceRequest
    let request = build_query_relationships_dance_request(node_collection, query_expression)
        .expect("Failed to build query_relationships request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.invoke_dance(request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "query_relationships request returned unexpected status: {}",
        response.description
    );
}
