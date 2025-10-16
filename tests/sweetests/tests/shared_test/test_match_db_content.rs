use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::info;

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};

use holons_prelude::prelude::*;

// use base_types::{MapInteger, MapString};
use core_types::HolonId; // TODO: Eliminate this dependency

use holons_client::init_client_context;
use holons_core::{core_shared_objects::ReadableHolonState, dances::ResponseBody};
// use holons_guest_integrity::HolonNode;

/// This function iterates through the expected_holons vector supplied as a parameter
/// and for each holon: builds and dances a `get_holon_by_id` DanceRequest,
/// then confirms that the Holon returned matches the expected

pub async fn execute_match_db_content(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Iterate through all created holons and verify them in the database
    for (_key, expected_holon) in test_state.created_holons.clone() {
        // Get HolonId
        let holon_id: HolonId = expected_holon.holon_id().expect("Failed to get HolonId").into();

        // 3. Build the get_holon_by_id DanceRequest
        let request = build_get_holon_by_id_dance_request(holon_id.clone())
            .expect("Failed to build get_holon_by_id request");

        info!("Dance Request: {:#?}", request);

        // 4. Call the dance
        let response = test_state.dance_call_service.dance_call(context, request).await;

        // 5. Ensure response contains the expected Holon
        if let ResponseBody::Holon(actual_holon) = response.body {
            assert_eq!(
                expected_holon.essential_content(),
                actual_holon.essential_content(),
                "Holon content mismatch for ID {:?}",
                holon_id
            );

            info!(
                "SUCCESS! DB fetched holon matched expected for: \n {:?}",
                actual_holon.summarize()
            );
        } else {
            panic!(
                "Expected get_holon_by_id to return a Holon response for id: {:?}, but got {:?}",
                holon_id, response.body
            );
        }
    }
}
