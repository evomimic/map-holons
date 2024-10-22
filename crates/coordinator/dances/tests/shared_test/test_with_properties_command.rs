
use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{Holons, Index};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
    build_with_properties_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::commit_manager::StagedIndex;
use rstest::*;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;

use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase, DanceTestStep};
use crate::shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `with_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) with_properties dance returns with a Success
/// 2) the returned index refers to a staged_holon that matches the expected_holon
///

pub async fn execute_with_properties(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    staged_holon_index: StagedIndex,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
) {
    info!("\n\n--- TEST STEP: with_properties Command:");
    // Get the state of the holon prior to dancing the request
    info!("trying to get staged_holon at staged_holon_index: {:#?}", staged_holon_index);

    info!("test state is: {:#?}", test_state );

    let original_holon = test_state
        .session_state
        .get_staging_area()
        .get_holon(staged_holon_index)
        .expect("Failed to get staged holon from test state");


    // Create the expected_holon from the original_holon + the supplied property values
    let mut expected_holon = original_holon.clone();
    for (property_name, base_value) in properties.clone() {
        let result = expected_holon.with_property_value(property_name.clone(), base_value.clone());
        if let Err(e) = result {
            panic!("Unable to add property value to expected holon, due to: {:#?}", e);
        }
    }
    // Build a with_properties DanceRequest
    let request = build_with_properties_dance_request(&test_state.session_state, staged_holon_index, properties.clone());
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            debug!("Dance Response: {:#?}", response.clone());
            let code = response.status_code;
            let description = response.description.clone();
            test_state.session_state = response.state.clone();
            assert_eq!(expected_response, code.clone());
            if let ResponseStatusCode::OK = code {
                if let Index(index) = response.body {
                    let index_value = index.to_string();
                    debug!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let actual_holon = &response
                        .state
                        .get_staging_area()
                        .get_holon(index)
                        .expect("Failed to get holon in response.");

                    assert_eq!(expected_holon, actual_holon.clone());

                    info!("Success! Holon has updated with supplied properties");
                    info!("test state is: {:#?}", test_state );
                }
            } else {
                panic!("DanceRequest returned {code} for {description}");
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a stage_holon request ", error);
        }
    }
}
