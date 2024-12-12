use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{Holons, Index};
use dances::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
use dances::descriptors_dance_adapter::build_load_core_schema_dance_request;
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;

use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `load_core_schema` DanceRequest
/// and confirms a Success response
///

pub async fn execute_load_new_schema(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
) -> () {
    info!("\n\n--- TEST STEP: Loading Core Schema:");
    // println!("{:#?}", expected_holon.clone());
    // Build a stage_holon DanceRequest
    let request = build_load_core_schema_dance_request(&test_state.session_state);
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            debug!("Dance Response: {:#?}", response.clone());
            let code = response.status_code;
            let description = response.description.clone();
            test_state.session_state = response.state.clone();
            if let ResponseStatusCode::OK = code {
                if let ResponseBody::None = response.body.clone() {
                    info!("Success! Load Schema Completed without error");
                } else {
                    panic!(
                        "Expected `None` in response body, but got {:#?} instead!",
                        response.body.clone()
                    );
                }
            } else {
                panic!("DanceRequest returned {code} for {description}");
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a load_core_schema request ", error);
        }
    }
}
