//! Holon Descriptor Test Cases

#![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{Holons, Index};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
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

/// This function builds and dances a `stage_new_holon` DanceRequest for the supplied Holon
/// and confirms a Success response
///

pub async fn execute_stage_new_holon(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    expected_holon: Holon,
) -> () {
    info!("\n\n--- TEST STEP: Staging a new Holon:");
    // println!("{:#?}", expected_holon.clone());
    // Build a stage_holon DanceRequest
    let request = build_stage_new_holon_dance_request(
        test_state.staging_area.clone(),
        expected_holon.clone(),
    );
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            debug!("Dance Response: {:#?}", response.clone());
            test_state.staging_area = response.staging_area.clone();
            let code = response.status_code;
            let description = response.description.clone();
            if let ResponseStatusCode::OK = code {
                if let Index(index) = response.body {
                    let index_value = index.to_string();
                    debug!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let holons = response.staging_area.staged_holons;
                    assert_eq!(expected_holon, holons[index as usize]);

                    info!("Success! Holon has been staged, as expected");
                } else {
                    panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
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
