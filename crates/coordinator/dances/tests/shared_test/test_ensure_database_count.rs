//! Holon Descriptor Test Cases

#![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody::Holons;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
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

/// This function builds and dances a `get_all_holons` DanceRequest and confirms that the number
/// of holons returned matches the expected_count of holons provided.
///

pub async fn execute_ensure_database_count(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    expected_count: MapInteger,

) {

    let expected_count_string = expected_count.0.to_string();
    info!("\n\n--- TEST STEP: Ensuring database holds {expected_count_string} holons ---");
    // Build a get_all_holons DanceRequest
    let request = build_get_all_holons_dance_request(test_state.staging_area.clone());
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            test_state.staging_area = response.staging_area.clone();

            if let Holons(holons) = response.body.clone() {
                assert_eq!(expected_count, MapInteger(holons.len() as i64));
                let actual_count = holons.len().to_string();

                info!("Success! DB has {actual_count} holons, as expected");

            } else {
                panic!(
                    "Expected get_all_holons to return Holons response, but it returned {:?}",
                    response.body
                );
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a get_all_holons request ", error);
        }
    }
}
