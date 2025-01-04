use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use async_std::task;
use dances::dance_response::ResponseBody::{Holons, StagedReference};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_add_related_holons_dance_request, build_get_all_holons_dance_request,
    build_with_properties_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::reference_layer::staged_reference::StagedIndex;
use holons::reference_layer::HolonReference;
use holons::shared_objects_layer::{Holon, RelationshipName};
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use std::collections::BTreeMap;

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holons
///

pub async fn execute_add_related_holons(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    source_holon_index: StagedIndex,
    relationship_name: RelationshipName,
    holons_to_add: Vec<HolonReference>,
    expected_response: ResponseStatusCode,
    expected_holon: Holon,
) -> () {
    info!("\n\n--- TEST STEP: Adding Related Holons. Expecting  {:#?}", expected_response.clone());

    // Ensure the source holon exists
    let _source_holon = test_state
        .session_state
        .get_staging_area()
        .get_holon(source_holon_index)
        .expect("Failed to get source holon from StagingArea");

    // Create the expected_holon from the source_holon + the supplied related holons

    // Build the DanceRequest
    let request = build_add_related_holons_dance_request(
        &test_state.session_state,
        source_holon_index,
        relationship_name,
        holons_to_add,
    );
    info!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            info!("Dance Response: {:#?}", response.clone());
            let code = response.status_code;

            test_state.session_state = response.state.clone();

            assert_eq!(code, expected_response);
            info!("as expected, add_related_holons dance request returned {:#?}", code.clone());

            if let ResponseStatusCode::OK = code {
                if let StagedReference(index) = response.body {
                    let index_value = index.to_string();
                    info!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let source_holon_in_response = response
                        .state
                        .get_staging_area()
                        .get_holon(index)
                        .expect("Failed to get source holon in response");

                    assert_eq!(source_holon_in_response, expected_holon);

                    info!("Success! Related Holons have been added");
                } else {
                }
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a stage_holon request ", error);
        }
    }
}
