use std::collections::BTreeMap;

use async_std::task;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{DanceTestExecutionState, DancesTestCase};
use crate::shared_test::*;
use holon_dance_builders::get_holon_by_id_dance::build_get_holon_by_id_dance_request;
use holons_client::init_client_context;
use holons_core::dances::ResponseBody;
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use tracing::info;

/// This function iterates through the expected_holons vector supplied as a parameter
/// and for each holon: builds and dances a `get_holon_by_id` DanceRequest,
/// then confirms that the Holon returned matches the expected

pub async fn execute_match_db_content(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");
    info!("test_state {:#?}", test_state);

    // 1. Get context from test_state
    let context = &*test_state.context;

    // 2. Iterate through all created holons and verify them in the database
    for (_key, expected_holon) in test_state.created_holons.clone() {
        // Get HolonId
        let holon_id: HolonId =
            expected_holon.get_local_id().expect("Failed to get local ID").into();

        // 3. Build the get_holon_by_id DanceRequest
        let request = build_get_holon_by_id_dance_request(holon_id.clone())
            .expect("Failed to build get_holon_by_id request");

        info!("Dance Request: {:#?}", request);

        // 4. Call the dance
        let response = test_state.dance_call_service.dance_call(context, request);

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
