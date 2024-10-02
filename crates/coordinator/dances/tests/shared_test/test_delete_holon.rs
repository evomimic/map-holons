use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::{Holons, Index};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_delete_holon_dance_request, build_get_all_holons_dance_request,
    build_get_holon_by_id_dance_request, build_stage_new_holon_dance_request,
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
use shared_types_holon::{HolonId, LocalId, MapInteger, MapString};

/// This function builds and dances a `delete_holon` DanceRequest for the supplied Holon
/// and matches the expected response
///

pub async fn execute_delete_holon(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    expected_response: ResponseStatusCode,
) -> () {
    info!("\n\n--- TEST STEP: Staging a new Holon:");
    // for now, there is always only 1 Holon in created_holons when testing a Delete,
    // until support for retrieval by key is implemented in Issue 118
    let holon_to_delete = &test_state.created_holons[0];
    let local_id = holon_to_delete.get_local_id().unwrap();
    // Build a stage_holon DanceRequest
    let delete_holon_request =
        build_delete_holon_dance_request(test_state.staging_area.clone(), local_id.clone());
    debug!("delete_holon Dance Request: {:#?}", delete_holon_request);

    match delete_holon_request {
        Ok(valid_request) => {
            let delete_holon_response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            debug!(
                "delete_holon Dance Response: {:#?}",
                delete_holon_response.clone()
            );
            let code = delete_holon_response.status_code;
            assert_eq!(
                code, expected_response,
                "Returned {:?} did not match expected {:?}",
                code, expected_response
            );
            let result = match expected_response {
                ResponseStatusCode::OK => Ok(()),
                _ => Err(code),
            };

            match result {
                Ok(_) => {
                    info!("Success! delete_holon returned OK response, confirming deletion...");
                    let get_holon_by_id_request = build_get_holon_by_id_dance_request(
                        test_state.staging_area.clone(),
                        HolonId::Local(local_id.clone()),
                    );
                    match get_holon_by_id_request {
                        Ok(valid_request) => {
                            let get_holon_by_id_response: DanceResponse = conductor
                                .call(&cell.zome("dances"), "dance", valid_request)
                                .await;

                            let code = get_holon_by_id_response.status_code;
                            assert_eq!(code, ResponseStatusCode::NotFound);
                        }
                        Err(holon_error) => {
                            panic!("{:?} Unable to build a stage_holon request ", holon_error);
                        }
                    }
                }
                Err(response) => {
                    info!(
                        "Success! delete_holon matched expected_response Error variant: {:?}",
                        response
                    );
                }
            }
        }
        Err(holon_error) => {
            panic!("{:?} Unable to build a stage_holon request ", holon_error);
        }
    }
}
