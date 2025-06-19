use async_std::task;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, info};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use rstest::*;

use crate::shared_test::*;
use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};
use base_types::{MapInteger, MapString};
use core_types::HolonId;
use holon_dance_builders::with_properties_dance::build_with_properties_dance_request;
use holons_core::{
    core_shared_objects::holon::HolonBehavior,
    dances::{ResponseBody, ResponseStatusCode},
    ReadableHolon, StagedReference,
};
use integrity_core_types::{HolonNode, PropertyMap, PropertyName};

use holon_dance_builders::with_properties_dance::build_with_properties_dance_request;

/// This function builds and dances a `with_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) with_properties dance returns with a Success
/// 2) the returned index refers to a staged_holon that matches the expected_holon
///

pub async fn execute_with_properties(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    original_holon: StagedReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Updating Holon with Properties ---");

    // 1. Get context from test_state
    let context = test_state.context();

    info!("Original Holon: {:?}", original_holon);

    // 3. Create the expected holon by applying the property updates
    let mut expected_holon = original_holon
        .clone_holon(context)
        .expect("Failed to clone original holon into expected holon");

    for (property_name, base_value) in properties.clone() {
        expected_holon
            .with_property_value(property_name.clone(), base_value.clone())
            .expect("Failed to add property value to expected holon");
    }

    // 4. Build the with_properties DanceRequest
    let request = build_with_properties_dance_request(original_holon, properties.clone())
        .expect("Failed to build with_properties request");

    debug!("Dance Request: {:#?}", request);

    // 5. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 6. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "with_properties request returned unexpected status: {}",
        response.description
    );

    // 7. If successful, verify the updated holon
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::StagedRef(updated_holon) = response.body {
            debug!("Updated holon reference returned: {:?}", updated_holon);

            assert_eq!(
                expected_holon.essential_content(),
                updated_holon.essential_content(context),
                "Updated Holon content did not match expected"
            );

            info!("Success! Holon has been updated with supplied properties.");
        } else {
            panic!("Expected StagedRef in response body, but got {:?}", response.body);
        }
    }
}
