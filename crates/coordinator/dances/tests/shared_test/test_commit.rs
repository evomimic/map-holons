//! Holon Descriptor Test Cases

#![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{self, Holons, Index};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_commit_dance_request, build_get_all_holons_dance_request,
    build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use crate::shared_test::data_types::DanceTestStep;
use crate::shared_test::data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(conductor: &SweetConductor, cell: &SweetCell, test_state: &mut DanceTestState) ->() {

    info!("\n\n--- TEST STEP: Committing Staged Holons ---- :");


    // Build a commit DanceRequest
    let request = build_commit_dance_request(test_state.staging_area.clone());
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
            if code == ResponseStatusCode::OK {
                // Check that staging area is empty
                assert!(response.staging_area.staged_holons.is_empty());

                info!("Success! Commit succeeded");

                // get saved holons out of response body and add them to the test_state created holons
                match response.body {
                    ResponseBody::Holon(holon) => {
                        test_state.created_holons.push(holon);
                    }
                    ResponseBody::Holons(holons) => {
                        for holon in holons {
                            test_state.created_holons.push(holon);
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
