use std::collections::BTreeMap;

use async_std::task;

// use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::nanoid::format;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase};
use crate::shared_test::*;
use holon_dance_builders::query_relationships_dance::build_query_relationships_dance_request;
use holons_core::dances::ResponseStatusCode;
use holons_core::query_layer::{Node, NodeCollection, QueryExpression};
use holons_core::{HolonReference, SmartReference};
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use tracing::{debug, info};

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
        HolonReference::Smart(SmartReference::new(HolonId::Local(source_holon_id), None));

    let node_collection =
        NodeCollection { members: vec![Node::new(holon_reference, None)], query_spec: None };

    // 3. Build the query_relationships DanceRequest
    let request = build_query_relationships_dance_request(node_collection, query_expression)
        .expect("Failed to build query_relationships request");

    debug!("Dance Request: {:#?}", request);

    // 4. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request);
    debug!("Dance Response: {:#?}", response.clone());

    // 5. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "query_relationships request returned unexpected status: {}",
        response.description
    );
}
