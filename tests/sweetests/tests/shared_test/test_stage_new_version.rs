use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, info};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use holon_dance_builders::stage_new_version_dance::build_stage_new_version_dance_request;
use holons_core::{
    core_shared_objects::{Holon, HolonBehavior},
    dances::{ResponseBody, ResponseStatusCode},
    reference_layer::{
        HolonCollectionApi, HolonReference, ReadableHolon, SmartReference, StagedReference,
    },
};

use base_types::MapString;
use core_types::{HolonError, HolonId};
use integrity_core_types::PropertyName;

use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{
        DanceTestExecutionState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
    },
};

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    original_holon_key: MapString,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Staging a New Version of a Holon ---");

    // 1. Get context from test_state
    let context = test_state.context();
    let staging_service = context.get_space_manager().get_staging_behavior_access();

    // 1. Retrieve the original Holon
    let original_holon =
        test_state.get_created_holon_by_key(&original_holon_key).unwrap_or_else(|| {
            panic!("Holon with key {:?} not found in created_holons", original_holon_key)
        });

    let original_holon_id =
        HolonId::Local(original_holon.get_local_id().expect("Failed to get LocalId"));

    // 2. Build the DanceRequest
    let request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(test_state.context(), request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );

    // 5. Extract the new version holon from the response
    let version_1: StagedReference = match response.status_code {
        ResponseStatusCode::OK => match response.body {
            ResponseBody::StagedRef(ref h) => h.clone(),
            _ => panic!("Expected StagedRef in response body, but got {:?}", response.body),
        },
        _ => panic!("Expected Ok response, but got {:?}", response.status_code),
    };

    debug!("New version Holon reference returned: {:?}", version_1);

    // 6. Verify the new version matches original version's essential content
    assert_eq!(
        original_holon.essential_content(),
        version_1.essential_content(context),
        "New version Holon content did not match original"
    );

    // 7. Verify the new version as the original holon as its predecessor
    let predecessor = version_1.get_predecessor(context).unwrap();

    assert_eq!(
        predecessor,
        Some(HolonReference::Smart(SmartReference::new(original_holon_id.clone(), None))),
        "Predecessor relationship did not match expected"
    );

    // 8. Verify new version's key matches original holon's key and that it is the ONLY staged
    // holon whose key matches.
    let by_base =
        staging_service.borrow().get_staged_holon_by_base_key(&original_holon_key).unwrap();

    assert_eq!(version_1, by_base, "get_staged_holon_by_base_key did not match expected");

    // 9. Verify staged holon retrieval by versioned key
    let by_version = staging_service
        .borrow()
        .get_staged_holon_by_versioned_key(&version_1.get_versioned_key(context).unwrap())
        .unwrap();

    assert_eq!(version_1, by_version, "get_staged_holon_by_versioned_key did not match expected");

    info!("Success! New version Holon matched expected content and relationships.");

    // Stage a second version from the same original holon in order to verify that:
    // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // b. get_staged_holons_by_base_key correctly returns BOTH stage holons
    let next_request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");
    debug!("2nd Dance Request: {:#?}", next_request);

    let next_response =
        test_state.dance_call_service.dance_call(test_state.context(), next_request).await;
    info!("2nd Dance Response: {:#?}", next_response.clone());

    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );

    // Extract the second new version holon from the response
    let version_2: StagedReference = match next_response.status_code {
        ResponseStatusCode::OK => match next_response.body {
            ResponseBody::StagedRef(ref h) => h.clone(),
            _ => panic!("Expected StagedRef in response body, but got {:?}", next_response.body),
        },
        _ => panic!("Expected Ok response, but got {:?}", next_response.status_code),
    };

    debug!("Second New version Holon reference returned: {:?}", version_2);

    // Ensure essential content is preserved
    assert_eq!(
        original_holon.essential_content(),
        version_2.essential_content(context),
        "New version Holon content did not match original"
    );

    // Confirm that get_staged_holon_by_versioned_key returns the new version
    let versioned_lookup = staging_service
        .borrow()
        .get_staged_holon_by_versioned_key(&version_2.get_versioned_key(context).unwrap())
        .unwrap();

    assert_eq!(
        version_2, versioned_lookup,
        "get_staged_holon_by_versioned_key did not match expected"
    );

    info!("Success! Second new version Holon matched expected content and relationships.");

    // Confirm that get_staged_holon_by_base_key returns a duplicate error.
    let book_holon_staged_reference_result = staging_service
        .borrow()
        .get_staged_holon_by_base_key(&original_holon_key)
        .expect_err("Expected duplicate error");
    assert_eq!(
        HolonError::DuplicateError(
            "Holons".to_string(),
            "key: Emerging World: The Evolution of Consciousness and the Future of Humanity"
                .to_string()
        ),
        book_holon_staged_reference_result
    );

    // Confirm that get_staged_holons_by_base_key returns two staged references for the two versions.
    let book_holon_staged_references =
        staging_service.borrow().get_staged_holons_by_base_key(&original_holon_key).unwrap();
    assert_eq!(
        book_holon_staged_references.len(),
        2,
        "get_staged_holons_by_base_key should return two staged references"
    );
    assert_eq!(
        vec![version_1, version_2],
        book_holon_staged_references,
        "Fetched staged references did not match expected"
    );
}
