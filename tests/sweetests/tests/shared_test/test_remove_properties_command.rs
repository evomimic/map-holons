use crate::mock_conductor::MockConductorConfig;
use holons_prelude::prelude::*;

// use holons_core::{
//     dances::{ResponseBody, ResponseStatusCode},
//     HolonReference, ReadableHolon, WritableHolon,
// };
use tracing::{debug, info};
// use holon_dance_builders::remove_properties_dance::build_remove_properties_dance_request;
use crate::shared_test::{
    // mock_conductor::MockConductorConfig,
    test_data_types::DanceTestExecutionState,
};

/// This function builds and dances a `remove_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) remove_properties dance returns with a Success
/// 2) the returned HolonReference refers to a Holon's essential_content that matches the expected
///

pub async fn execute_remove_properties(
    test_state: &mut DanceTestExecutionState,
    original_holon: HolonReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Removing Properties from Holon ---");

    // 1. Get context from test_state
    let ctx_arc = test_state.context(); // Arc lives until end of scope
    let context = ctx_arc.as_ref();

    info!("Original Holon: {:?}", original_holon);

    // 3. Create the expected holon by applying the property updates
    let mut expected_holon = original_holon.clone();

    for property_name in properties.keys() {
        expected_holon
            .remove_property_value(context, property_name)
            .expect("Failed to remove property value from expected holon");
    }

    // 4. Build the remove_properties DanceRequest
    let request = build_remove_properties_dance_request(original_holon, properties.clone())
        .expect("Failed to build remove_properties request");

    debug!("Dance Request: {:#?}", request);

    // 5. Call the dance
    let response = test_state.invoke_dance(request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 6. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "remove_properties request returned unexpected status: {}",
        response.description
    );

    // 7. If successful, verify the updated holon
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::HolonReference(updated_holon) = response.body {
            assert_eq!(
                expected_holon.essential_content(context),
                updated_holon.essential_content(context),
                "Updated Holon content did not match expected"
            );

            info!("Success! Supplied properties have been removed from the Holon.");
        } else {
            panic!("Expected HolonReference in response body, but got {:?}", response.body);
        }
    }
}
