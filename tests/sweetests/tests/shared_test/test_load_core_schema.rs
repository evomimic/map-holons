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
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};

use holons_prelude::prelude::*;

use holons_core::dances::descriptors_dance_adapter::build_load_core_schema_dance_request;

/// This function builds and dances a `load_core_schema` DanceRequest
/// and confirms a Success response
///
pub async fn execute_load_new_schema(
    // test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    test_state: &mut DanceTestExecutionState,
) {
    info!("--- TEST STEP: Loading Core Schema ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build the load_core_schema DanceRequest
    let request =
        build_load_core_schema_dance_request().expect("Failed to build load_core_schema request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "load_core_schema request returned unexpected status: {}",
        response.description
    );

    // 5. Ensure response body is None (successful schema load should not return a body)
    assert!(
        matches!(response.body, ResponseBody::None),
        "Expected `None` in response body, but got {:#?} instead!",
        response.body
    );

    info!("Success! Load Schema Completed without error.");
}
