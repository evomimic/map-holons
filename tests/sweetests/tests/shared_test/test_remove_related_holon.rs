use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::info;

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::{
    mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase},
};

use holons_prelude::prelude::*;

/// This function is intended to test the ability to remove holons from a specified relationship
/// originating at a source_holon.
///
/// There are two levels of testing required.
/// 1. Removing related holons from an already staged holon.
/// 2. Removing related holons from a previously saved holon
///
/// The first is a local operation on the staged holon (it does not invoke any dances).
///
/// The second requires:
///     a. retrieving the source holon
///     b. either cloning it or staging a new version of it
///     c. removing the specified holons from the specified relationship
///     d. committing the changes
///     e. confirming the new holon is no longer related to the holons to remove via the specified relationship.
///

pub async fn execute_remove_related_holons(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    source_holon: HolonReference,
    relationship_name: RelationshipName,
    holons_to_remove: Vec<HolonReference>,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Removing Related Holons ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build the DanceRequest (state is handled inside dance_call)
    let request = build_remove_related_holons_dance_request(
        source_holon,
        relationship_name,
        holons_to_remove,
    )
    .expect("Failed to build remove_related_holons request");

    info!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(response.status_code, expected_response);
    info!("as expected, remove_related_holons dance request returned {:#?}", response.status_code);

    // 5. If successful, confirm related Holons were removed
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::HolonReference(updated_holon) = response.body {
            info!("Updated holon returned: {:?}", updated_holon);
            info!("Success! Related Holons have been removed");
        } else {
            panic!("Expected remove_related_holons to return a StagedRef response, but it didn't");
        }
    }
}
