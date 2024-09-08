// #![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_get_holon_by_id_dance_request,
    build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::context::HolonsContext;
use rstest::*;

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

/// This function iterates through the expected_holons vector supplied as a parameter
/// and for each holon: builds and dances a `get_holon_by_id` DanceRequest,
/// then confirms that the Holon returned matches the expected

pub async fn execute_match_db_content(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
) {
    info!("\n\n--- TEST STEP: Ensuring database matches expected holons ---");
    let context = HolonsContext::new(); // initialize empty context to satisfy get_key() unused param in HolonGettable trait

    for expected_holon in test_state.created_holons.clone() {
        // get HolonId
        let holon_id : HolonId= expected_holon.get_local_id().unwrap().into();
        // Build a get_holon_by_id DanceRequest
        let request =
            build_get_holon_by_id_dance_request(test_state.staging_area.clone(), holon_id.clone());
        info!("Dance Request: {:#?}", request);

        match request {
            Ok(valid_request) => {
                let response: DanceResponse = conductor
                    .call(&cell.zome("dances"), "dance", valid_request)
                    .await;
                test_state.staging_area = response.staging_area.clone();

                if let ResponseBody::Holon(actual_holon) = response.body.clone() {
                    assert_eq!(
                        expected_holon.essential_content(),
                        actual_holon.essential_content(),
                    );
                    info!("Success! DB fetched holon matched expected");
                } else {
                    panic!(
                        "Expected get_holon_by_id to return a Holon response for id: {:?}, but it returned {:?}",
                        holon_id,
                        response.body
                    );
                }
            }
            Err(error) => {
                panic!("{:?} Unable to build a get_holon_by_id request ", error);
            }
        }
    }
}
