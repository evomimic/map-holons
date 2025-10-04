use async_std::task;
use std::collections::BTreeMap;
use tracing::{debug, info, warn};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons_core::core_shared_objects::HolonBehavior; // TODO: eliminate this dependency

use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};

use holons_prelude::prelude::*;

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(test_state: &mut DanceTestExecutionState<MockConductorConfig>) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build commit DanceRequest (state is handled inside dance_call)
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "Commit request failed: {}",
        response.description
    );
    info!("Success! Commit succeeded");

    // 5. Extract saved Holons from response body and add them to `created_holons`
    match response.body {
        ResponseBody::Holon(holon) => {
            let key =
                holon.get_key().expect("Holon should have a key").expect("Key should not be None");
            test_state.created_holons.insert(key, holon);
        }
        ResponseBody::Holons(holons) => {
            for holon in holons {
                let key = holon
                    .get_key()
                    .expect("Holon should have a key")
                    .expect("Key should not be None");
                test_state.created_holons.insert(key, holon);
            }
        }
        _ => panic!("Invalid ResponseBody: {:?}", response.body),
    }
}
