use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, info};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::*;
use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};

use base_types::{MapInteger, MapString};
use core_types::HolonId;
use core_types::{PropertyMap, PropertyName};
use holon_dance_builders::query_relationships_dance::build_query_relationships_dance_request;
use holons_core::{
    core_shared_objects::ReadableHolonState,
    dances::ResponseStatusCode,
    query_layer::{Node, NodeCollection, QueryExpression},
    HolonReference, ReadableHolon, SmartReference, WritableHolon,
};

// use holons_guest_integrity::HolonNode;

/// This function builds and dances a `query_relationships` DanceRequest for the supplied NodeCollection and QueryExpression.
pub async fn execute_query_relationships(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    source_key: MapString,
    query_expression: QueryExpression,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Querying Relationships ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Retrieve the source Holon
    let source_holon = test_state
        .get_created_holon_by_key(&source_key)
        .unwrap_or_else(|| panic!("Holon with key {:?} not found in created_holons", source_key));

    let source_holon_id = source_holon
        .get_local_id()
        .expect(&format!("Failed to get local_id for Holon: {:#?}", source_holon));

    let holon_reference =
        HolonReference::Smart(SmartReference::new_from_id(HolonId::Local(source_holon_id)));

    let node_collection =
        NodeCollection { members: vec![Node::new(holon_reference, None)], query_spec: None };

    // 3. Build the query_relationships DanceRequest
    let request = build_query_relationships_dance_request(node_collection, query_expression)
        .expect("Failed to build query_relationships request");

    debug!("Dance Request: {:#?}", request);

    // 4. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 5. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "query_relationships request returned unexpected status: {}",
        response.description
    );
}
