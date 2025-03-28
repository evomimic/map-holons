use std::collections::BTreeMap;

use async_std::task;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons_core::dances::descriptors_dance_adapter::build_load_core_schema_dance_request;

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{DanceTestExecutionState, DancesTestCase};
use crate::shared_test::*;
use holons_core::dances::{DanceResponse, ResponseBody, ResponseStatusCode};
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use tracing::{debug, info};
/// This function builds and dances a `load_core_schema` DanceRequest
/// and confirms a Success response
///

pub async fn execute_load_new_schema(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
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
