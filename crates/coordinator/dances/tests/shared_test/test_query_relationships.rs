use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::Collection;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_query_relationships_dance_request,
    build_stage_new_holon_dance_request, build_with_properties_dance_request,
};
use hdk::prelude::*;
use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::nanoid::format;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::smart_reference::SmartReference;
use rstest::*;

use crate::shared_test::test_data_types::{DanceTestState, DanceTestStep, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use holons::query::{Node, NodeCollection, QueryExpression};
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `query_relationships` DanceRequest for the supplied NodeCollection and QueryExpression.
pub async fn execute_query_relationships(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    source_key: MapString,
    query_expression: QueryExpression,
    expected_response: ResponseStatusCode,
) {
    info!("\n\n--- TEST STEP: query_relationships QueryCommand:");

    let source_holon = test_state
        .created_holons
        .get(&source_key)
        .expect("Holon with key: {source_key} not found in created_holons");

    let source_holon_id = source_holon
        .get_local_id()
        .expect(&format!("Get local_id for Holon: {:#?} \n returned error:", source_holon));

    let holon_reference: HolonReference =
        HolonReference::Smart(SmartReference::new(HolonId::Local(source_holon_id), None));

    let node_collection =
        NodeCollection { members: vec![Node::new(holon_reference, None)], query_spec: None };

    let request = build_query_relationships_dance_request(
        &test_state.session_state,
        node_collection,
        query_expression,
    )
    .expect("Unable to build a query_relationships request, got:");
    debug!("Dance Request: {:#?}", request);

    let response: DanceResponse = conductor.call(&cell.zome("dances"), "dance", request).await;
    debug!("Dance Response: {:#?}", response.clone());
    let code = response.status_code;
    let description = response.description.clone();
    test_state.session_state = response.state.clone();
    assert_eq!(expected_response, code, "DanceRequest returned {:?} for {:?}", code, description);
}
