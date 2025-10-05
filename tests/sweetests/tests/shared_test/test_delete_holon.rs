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
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};

use holon_dance_builders::delete_holon_dance::build_delete_holon_dance_request;
use holon_dance_builders::get_holon_by_id_dance::build_get_holon_by_id_dance_request;

use holons_core::{core_shared_objects::ReadableHolonState, dances::ResponseStatusCode};

use base_types::{MapInteger, MapString};
use core_types::HolonId;
use core_types::{LocalId, PropertyMap, PropertyName};

/// This function builds and dances a `delete_holon` DanceRequest for the supplied Holon
/// and matches the expected response
///
pub async fn execute_delete_holon(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    holon_to_delete_key: MapString, // key of the holon to delete
    expected_response: ResponseStatusCode,
) {
    info!(
        "--- TEST STEP: Deleting an Existing (Saved) Holon with key: {:#?}",
        holon_to_delete_key.clone()
    );

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Retrieve the Holon to delete
    let holon_to_delete = test_state
        .get_created_holon_by_key(&holon_to_delete_key)
        .expect("Failed to retrieve holon from test_state's created_holons.");

    let local_id =
        holon_to_delete.get_local_id().expect("Unable to get LocalId from holon_to_delete");

    // 3. Build the delete Holon request
    let request = build_delete_holon_dance_request(local_id.clone())
        .expect("Failed to build delete_holon request");

    debug!("delete_holon Dance Request: {:#?}", request);

    // 4. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("delete_holon Dance Response: {:#?}", response.clone());

    // 5. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "Returned {:?} did not match expected {:?}",
        response.status_code, expected_response
    );

    if expected_response == ResponseStatusCode::OK {
        info!("Success! delete_holon returned OK response, confirming deletion...");

        // 6. Verify the holon no longer exists
        let get_request = build_get_holon_by_id_dance_request(HolonId::Local(local_id.clone()))
            .expect("Failed to build get_holon_by_id request");

        let get_response = test_state.dance_call_service.dance_call(context, get_request).await;
        assert_eq!(
            get_response.status_code,
            ResponseStatusCode::NotFound,
            "Holon should be deleted but was found"
        );

        info!("Confirmed Holon deletion!");
    } else {
        info!("delete_holon matched expected response: {:?}", response.status_code);
    }
}
