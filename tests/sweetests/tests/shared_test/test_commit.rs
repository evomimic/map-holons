use crate::mock_conductor::MockConductorConfig;
use async_std::task;
use holons_prelude::prelude::*;
use rstest::*;
use std::collections::BTreeMap;
use tracing::{debug, info, warn};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::{
    // mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};
use holons_prelude::prelude::*;

// TODO: Remove this import, direct access to HolonState should not be allowed from test layer.
// The need for this will go away once Holon is removed from ResponseBody

use holons_core::core_shared_objects::ReadableHolonState;
// use base_types::{MapInteger, MapString};
// use core_types::HolonId;
// use holon_dance_builders::commit_dance::build_commit_dance_request;
// use holons_core::{
//     core_shared_objects::ReadableHolonState,
//     dances::{ResponseBody, ResponseStatusCode},
// };
// // use holons_guest_integrity::HolonNode;
// use core_types::{PropertyMap, PropertyName};

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(test_state: &mut DanceTestExecutionState) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    // 1. Build commit DanceRequest (state is handled inside dance_call)
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 2. Call the dance
    let response = test_state.invoke_dance(request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. Validate response status
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "Commit request failed: {}",
        response.description
    );
    info!("Success! Commit succeeded");

    // 4. Extract saved Holons from response body and add them to `created_holons`
    match response.body {
        ResponseBody::Holon(holon) => {
            let key =
                holon.key().expect("Holon should have a key").expect("Key should not be None");
            test_state.created_holons.insert(key, holon);
        }
        ResponseBody::Holons(holons) => {
            for holon in holons {
                let key =
                    holon.key().expect("Holon should have a key").expect("Key should not be None");
                test_state.created_holons.insert(key, holon);
            }
        }
        _ => panic!("Invalid ResponseBody: {:?}", response.body),
    }
}
