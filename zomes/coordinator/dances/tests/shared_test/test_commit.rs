use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{self, Holons, StagedRef};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_commit_dance_request, build_get_all_holons_dance_request,
    build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use crate::shared_test::test_data_types::{DanceTestExecutionState, DancesTestCase};
use crate::shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestExecutionState,
) -> () {
    info!("\n\n--- TEST STEP: Committing Staged Holons ---- :");

    // Build a commit DanceRequest
    let request = build_commit_dance_request(&test_state.session_state);
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;

            debug!("Dance Response: {:#?}", response.clone());
            test_state.session_state = response.state.clone();
            let code = response.status_code;
            let description = response.description.clone();
            if code == ResponseStatusCode::OK {
                // Check that staging area is empty
                // assert!(response.state.get_staging_area().get_staged_holons().is_empty());

                info!("Success! Commit succeeded");

                // get saved holons out of response body and add them to the test_state created holons
                match response.body {
                    ResponseBody::Holon(holon) => {
                        let key = holon.get_key().unwrap().unwrap(); // test Holons should always have a key
                        test_state.created_holons.insert(key, holon);
                    }
                    ResponseBody::Holons(holons) => {
                        for holon in holons {
                            let key = holon.get_key().unwrap().unwrap();
                            test_state.created_holons.insert(key, holon);
                        }
                    }
                    _ => panic!("Invalid ResponseBody: {:?}", response.body),
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
