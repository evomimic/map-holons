use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, info};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::*;
use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{
        DanceTestExecutionState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
    },
};
use base_types::{MapInteger, MapString};
use core_types::HolonId;
use holon_dance_builders::stage_new_holon_dance::build_stage_new_holon_dance_request;
use holons_client::init_client_context;
use holons_core::{
    core_shared_objects::holon::{Holon, HolonBehavior, TransientHolon},
    dances::{ResponseBody, ResponseStatusCode},
    HolonsContextBehavior, ReadableHolon, StagedReference,
};
use integrity_core_types::{PropertyMap, PropertyName};
use holons_guest_integrity::HolonNode;

/// This function stages a new holon. It builds and dances a `stage_new_holon` DanceRequest for the
/// supplied Holon and confirms a Success response
///
pub async fn execute_stage_new_holon(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    transient_holon: TransientHolon,
) {
    info!("--- TEST STEP: Staging a new Holon via DANCE ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build the DanceRequest
    let request = build_stage_new_holon_dance_request(transient_holon.clone())
        .expect("Failed to build stage_new_holon request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "stage_new_holon request failed: {}",
        response.description
    );

    // 5. Verify the staged Holon
    if let ResponseBody::StagedRef(staged_holon) = response.body {
        debug!("Staged holon reference returned: {:#?}", staged_holon);

        assert_eq!(
            transient_holon.essential_content(),
            staged_holon.essential_content(context),
            "Staged Holon content did not match expected"
        );

        info!("Success! Holon has been staged as expected.");
    } else {
        panic!("Expected StagedRef in response body, but got {:?}", response.body);
    }

    // 6. Update the key_suffix_count
    test_state.key_suffix_count += 1;
}
