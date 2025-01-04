use std::collections::BTreeMap;

use async_std::task;
// use dances::dance_request::DanceRequest;
use dances::dance_response::ResponseBody::Holons;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_get_all_holons_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
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
    let actual_count_string = "0".to_string();
    info!("\n\n--- TEST STEP: Ensuring database holds {expected_count_string} holons ---");
    // Build a get_all_holons DanceRequest
    let request = build_get_all_holons_dance_request(&test_state.session_state);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            test_state.session_state = response.state;

            if let Holons(holons) = response.body.clone() {
                let actual_count = MapInteger(holons.len() as i64);
                info!(
                    "\n--- TEST STEP ensure_db_count: Expected: {:?}, Retrieved: {:?} Holons",
                    expected_count, actual_count.0
                );
                for holon in holons {
                    info!("\n {:?}", holon.summarize());
                }

                assert_eq!(expected_count, actual_count);
            } else {
                panic!(
                    "Expected get_all_holons to return {:?} holons, but it returned {:?}",
                    expected_count_string, actual_count_string
                );
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a get_all_holons request ", error);
        }
    }
}
