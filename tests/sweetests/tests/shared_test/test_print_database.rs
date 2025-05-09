use std::collections::BTreeMap;

use async_std::task;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase};
use crate::shared_test::*;

use crate::shared_test::mock_conductor::MockConductorConfig;
use holon_dance_builders::get_all_holons_dance::build_get_all_holons_dance_request;
use holons_core::dances::ResponseBody;
use holons_core::utils::as_json;
use rstest::*;
use shared_types_holon::holon_node::HolonNode;
use base_types::{MapInteger, MapString};
use core_types::{HolonId, PropertyMap, PropertyName};
use tracing::{debug, info};

/// This function retrieves all holons and then writes log messages for each holon:
/// `info!` -- writes only the "key" for each holon
/// `debug!` -- writes the full json-formatted contents of the holon
///

pub async fn execute_database_print(test_state: &mut DanceTestExecutionState<MockConductorConfig>) {
    info!("--- TEST STEP: Print Database Contents ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build the get_all_holons DanceRequest
    let request =
        build_get_all_holons_dance_request().expect("Failed to build get_all_holons request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. Verify response contains Holons
    if let ResponseBody::Holons(holons) = response.body {
        info!("DB contains {} holons", holons.len());

        for holon in holons {
            let key = holon
                .get_key()
                .map(|key| key.unwrap_or_else(|| MapString("<None>".to_string())))
                .unwrap_or_else(|err| {
                    panic!("Attempt to get_key() resulted in error: {:?}", err);
                });

            info!("Key = {:?}", key.0);
            info!("{:?}", holon.summarize());
            debug!("Holon JSON: {:?}", as_json(&holon));
        }
    } else {
        panic!("Expected get_all_holons to return Holons response, but got {:?}", response.body);
    }
}
