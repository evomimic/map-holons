use std::collections::BTreeMap;

use crate::shared_test::test_data_types::{
    DanceTestState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
};
use crate::shared_test::*;
use async_std::task;
use dances::dance_response::ResponseBody::Holons;
use dances::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use holons_client::init_client_context;
use holons_core::core_shared_objects::Holon;
use holons_core::{HolonsContextBehavior, StagedReference};
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function stages a new holon. If `local_only` is true, the holon is only staged in the local
/// nursery. Otherwise, this function builds and dances a `stage_new_holon` DanceRequest for the
/// supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_holon(
    _conductor: &SweetConductor,
    _cell: &SweetCell,
    // _test_state: &mut DanceTestState,
    context: &dyn HolonsContextBehavior,
    expected_holon: Holon,
    local_only: bool,
) -> () {
    if local_only {
        info!("\n\n--- TEST STEP: Staging a new Holon: LOCAL ONLY");
        let space_manager = &*context.get_space_manager();
        let staging_service = space_manager.get_staging_behavior_access();
        staging_service.borrow().stage_new_holon(context, expected_holon).unwrap();
        return;
    }

    // info!("\n\n--- TEST STEP: Staging a new Holon: DANCE -- NOT CURRENTLY IMPLEMENTED");
    //
    // // Build a stage_holon DanceRequest
    // let request =
    //     build_stage_new_holon_dance_request(&_test_state.session_state, expected_holon.clone());
    // debug!("Dance Request: {:#?}", request);
    //
    // match request {
    //     Ok(valid_request) => {
    //         let response: DanceResponse =
    //             _conductor.call(&_cell.zome("dances"), "dance", valid_request).await;
    //         debug!("Dance Response: {:#?}", response.clone());
    //         _test_state.session_state = response.state.clone();
    //         let code = response.status_code;
    //         let description = response.description.clone();
    //         if let ResponseStatusCode::OK = code {
    //             if let ResponseBody::StagedRef(index) = response.body {
    //                 let index_value = index.to_string();
    //                 debug!("{index_value} returned in body");
    //                 // An index was returned in the body, retrieve the Holon at that index within
    //                 // the StagingArea and confirm it matches the expected Holon.
    //                 let actual_holon = _test_state
    //                     .session_state
    //                     .get_staging_area()
    //                     .get_holon(index as usize)
    //                     .expect("Failed to get holon in response.");
    //
    //                 assert_eq!(expected_holon, actual_holon);
    //
    //                 info!("Success! Holon has been staged, as expected");
    //             } else {
    //                 panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
    //             }
    //         } else {
    //             panic!("DanceRequest returned {code} for {description}");
    //         }
    //     }
    //     Err(error) => {
    //         panic!("{:?} Unable to build a stage_holon request ", error);
    //     }
    // }
}
