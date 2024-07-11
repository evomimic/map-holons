// #![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::Collection;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_query_relationships_dance_request,
    build_stage_new_holon_dance_request, build_with_properties_dance_request, Node, NodeCollection,
    QueryExpression,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::smart_reference::SmartReference;
use rstest::*;

use crate::shared_test::dance_fixtures::*;
use crate::shared_test::test_data_types::DanceTestStep;
use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `query_relationshipss` DanceRequest for the supplied NodeCollection and QueryExpression.
pub async fn execute_query_relationships(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    source_key: MapString,
    query_expression: QueryExpression,
    expected_response: ResponseStatusCode,
) {
    info!("\n\n--- TEST STEP: query_relationships QueryCommand:");

    let dummy_context = HolonsContext::new();
    let source_holon_id =
        get_holon_by_key_from_test_state(&dummy_context, source_key.clone(), test_state);
    match source_holon_id {
        Ok(holon_id) => {
            if let Some(id) = holon_id {
                let holon_reference: HolonReference = HolonReference::Smart(SmartReference {
                    holon_id: id,
                    smart_property_values: None,
                });

                let node_collection = NodeCollection {
                    members: vec![Node::new(holon_reference, None)],
                    query_spec: None,
                };

                let request = build_query_relationships_dance_request(
                    test_state.staging_area.clone(),
                    node_collection,
                    query_expression,
                );
                debug!("Dance Request: {:#?}", request);

                match request {
                    Ok(valid_request) => {
                        let response: DanceResponse = conductor
                            .call(&cell.zome("dances"), "dance", valid_request)
                            .await;
                        debug!("Dance Response: {:#?}", response.clone());
                        let code = response.status_code;
                        let description = response.description.clone();
                        test_state.staging_area = response.staging_area.clone();

                        if let ResponseStatusCode::OK = code {
                            if let Collection(_node_collection) = response.body {
                                info!("Success! NodeCollection returned");
                            }
                        } else {
                            panic!("DanceRequest returned {code} for {description}");
                        }
                        assert_eq!(expected_response, code.clone());
                    }
                    Err(error) => {
                        panic!("{:?} Unable to build a query_relationships request ", error);
                    }
                }
            } else {
                panic!("Failed to get Holon by key:{:?}", source_key)
            }
        }
        Err(e) => panic!("get_holon_by_key_from_test_state returned error: {:?}", e),
    }
}
