use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::info;

use rstest::*;

use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase, TestReference},
};
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use holons_prelude::prelude::*;

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holon references. Accepting holons_to_add as TestReferences allows the target holons to
/// be either StagedHolons or SavedHolons. In the latter case, the executor needs to resolve
/// the TestReference's key into a HolonReference
///

pub async fn execute_add_related_holons(
    // test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    test_state: &mut DanceTestExecutionState,
    source_holon: HolonReference,
    relationship_name: RelationshipName,
    holons_to_add: Vec<TestReference>,
    expected_response: ResponseStatusCode,
    expected_holon: HolonReference,
) {
    info!("--- TEST STEP: Add Related Holons ---");

    // 1. Get the context from test_state
    let context = test_state.context();

    // 2. Build the DanceRequest (state is handled inside dance_call)
    let references_to_add = test_state
        .resolve_test_reference_vector(&holons_to_add)
        .expect("Failed to resolve one or more TestReferences in execute_add_related_holons");

    let request = build_add_related_holons_dance_request(
        source_holon.clone(),
        relationship_name,
        references_to_add,
    )
    .expect("Failed to build add_related_holons request");

    info!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(response.status_code, expected_response);
    info!("as expected, add_related_holons dance request returned {:#?}", response.status_code);

    // 5. If successful, validate that the related Holons were added correctly
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::HolonReference(actual_holon) = response.body {
            assert_eq!(
                actual_holon.essential_content(context),
                expected_holon.essential_content(context),
                "Expected holon did not match response holon"
            );
            info!("Success! Related Holons have been added");
        } else {
            panic!("Expected add_related_holons to return a None response, but it didn't");
        }
    }
}
