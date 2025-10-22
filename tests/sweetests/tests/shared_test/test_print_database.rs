use async_std::task;
use holons_core::MockConductorConfig;
use holons_prelude::prelude::*;
use pretty_assertions::assert_eq;
use rstest::*;
use std::collections::BTreeMap;
use tracing::{debug, info};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::{
    // mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};

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
    if let ResponseBody::HolonCollection(holons) = response.body {
        info!("DB contains {} holons", holons.get_count());

        for holon in holons {
            let key = holon
                .key(context)
                .map(|key| key.unwrap_or_else(|| MapString("<None>".to_string())))
                .unwrap_or_else(|err| {
                    panic!("Attempt to key() resulted in error: {:?}", err);
                });

            info!("Key = {:?}", key.0);
            info!("{:?}", holon.summarize(context));
            // debug!("Holon JSON: {:?}", as_json(&holon));
        }
    } else {
        panic!("Expected get_all_holons to return Holons response, but got {:?}", response.body);
    }
}
