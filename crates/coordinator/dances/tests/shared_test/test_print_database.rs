// #![allow(unused_imports)]

use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody;
use dances::dance_response::ResponseBody::Holons;
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

use crate::shared_test::dance_fixtures::*;
use crate::shared_test::test_data_types::DanceTestStep;
use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use holons::json_adapter::as_json;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function retrieves all holons and then writes log messages for each holon:
/// `info!` -- writes only the "key" for each holon
/// `debug!` -- writes the full json-formatted contents of the holon
///

pub async fn execute_database_print(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
) {
    info!("\n\n--- TEST STEP: print database contents ---");

    // Build a get_all_holons DanceRequest
    let request = build_get_all_holons_dance_request(&test_state.session_state);
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            test_state.session_state = response.state.clone();

            if let Holons(holons) = response.body.clone() {
                let actual_count = holons.len().to_string();
                info!("DB has {actual_count} holons");
                for holon in holons {
                    let key_result = holon.get_key();
                    match key_result {
                        Ok(key) => {
                            info!(
                                "key = {:?}",
                                key.unwrap_or_else(|| MapString("<None>".to_string())).0
                            );
                            debug!("\nHolon {:?}", as_json(&holon));
                        }
                        Err(holon_error) => {
                            panic!("Attempt to get_key() resulted in error {:?}", holon_error,);
                        }
                    }
                }
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
