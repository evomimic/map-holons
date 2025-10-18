use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::info;

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::*;
use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};

use holons_prelude::prelude::*;

/// This function builds and dances a `get_all_holons` DanceRequest and confirms that the number
/// of holons returned matches the expected_count of holons provided.
///

pub async fn execute_ensure_database_count(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    expected_count: MapInteger,
) {
    info!("--- TEST STEP: Ensuring database holds {} holons ---", expected_count.0);

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build the get_all_holons DanceRequest
    let request =
        build_get_all_holons_dance_request().expect("Failed to build get_all_holons request");

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;

    // 4. Verify response contains Holons
    if let ResponseBody::HolonCollection(holon_collection) = response.body {
        let actual_count = holon_collection.get_count();
        info!(
            "--- TEST STEP ensure_db_count: Expected: {:?}, Retrieved: {:?} Holons ---",
            expected_count, actual_count.0
        );

        // 5. Assert that the expected count matches actual count
        assert_eq!(expected_count, actual_count);
    } else {
        panic!(
            "Expected get_all_holons to return {} holons, but it returned an unexpected response: {:?}",
            expected_count.0, response.body
        );
    }
}
