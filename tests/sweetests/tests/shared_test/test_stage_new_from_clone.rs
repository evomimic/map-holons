use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, error, info, warn};

use rstest::*;

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{
    DanceTestExecutionState, DanceTestStep, DancesTestCase, TestReference,
};
use base_types::{MapInteger, MapString};
use core_types::HolonId;
use core_types::HolonId;
use core_types::{PropertyName, RelationshipName};
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holon_dance_builders::stage_new_from_clone_dance::build_stage_new_from_clone_dance_request;
use holons_core::{
    core_shared_objects::ReadableHolonState,
    dances::{ResponseBody, ResponseStatusCode},
    reference_layer::{HolonReference, ReadableHolon, SmartReference, TransientReference},
};
use holons_prelude::prelude::*;

/// This function builds and dances a `stage_new_from_clone` DanceRequest for the supplied
/// TestReference and confirms a Success response.
///
/// The implementation of this step consists of the following stages:
///      1. Construct the HolonReference to the original holon required by the DanceRequest
///      2. Build the DanceRequest
///      3. Dance the DanceRequest
///      4. Confirm the actual result matches the expect result
///
/// The `stage_new_from_clone_dance` is a Clone method that uses a `HolonReference` to identify the
/// Holon to clone. This means that to build the dance request, we need to create a `HolonReference`
/// from the `original_test_ref :  TestReference`.
///
/// The `original_test_ref` can either be a:
/// - `Saved` variant that holds the key for the previously saved holon to clone
/// - `Staged` variant that holds a StagedReference to the Nursery resident holon to clone.
/// - `Transient` variant that holds a TransientReference to the TransientHolonManager resident holon to clone.
///
/// To get the `HolonReference` for the `Saved` case, we need to:
///      1. retrieve the holon via its key from the test_state
///      2. get its `HolonId`
///      3. create a `SmartReference` for the `HolonId` and wrap the `SmartReference` in a `HolonReference`
///
///  To get the `HolonReference` in the `Staged case`, we simply need to wrap the `StagedReference`
///  in a `HolonReference`
pub async fn execute_stage_new_from_clone(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    original_test_reference: TestReference,
    new_key: MapString,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Cloning a Holon ---");

    // 1. Get context from test_state
    let context = test_state.context();

    info!("Got context from test_state");

    // 2. Construct the HolonReference to the original holon
    let original_holon_reference: HolonReference = match original_test_reference.clone() {
        TestReference::TransientHolon(transient_reference) => {
            HolonReference::Transient(transient_reference)
        }
        TestReference::StagedHolon(staged_reference) => HolonReference::Staged(staged_reference),
        TestReference::SavedHolon(key) => {
            let saved_holon = test_state
                .get_created_holon_by_key(&key)
                .unwrap_or_else(|| panic!("Holon with key {key} not found in created_holons"));

            let local_id = saved_holon.get_local_id().expect("Failed to get LocalId");
            HolonReference::Smart(SmartReference::new(
                HolonId::Local(local_id),
                Some(saved_holon.into_node().property_map.clone()),
            ))
        }
    };

    // TODO: fix in future issue for being able to clone from a Saved Holon
    // where the plan is to drop test execution state and change TestReference to HolonReference
    //
    // // Get the original holon (for comparison purposes)
    // let original_holon: Holon = match original_test_ref {
    //     TestReference::TransientHolon(transient_reference) => {
    //         match transient_reference.clone_holon(context) {
    //             Ok(holon) => Holon::Transient(holon),
    //             Err(err) => {
    //                 error!("Failed to clone holon: {:?}", err);
    //                 return; // or continue/fallback logic as appropriate
    //             }
    //         }
    //     }
    //     TestReference::StagedHolon(staged_reference) => {
    //         match staged_reference.clone_holon(context) {
    //             Ok(holon) => Holon::Transient(holon),
    //             Err(err) => {
    //                 error!("Failed to clone holon: {:?}", err);
    //                 return; // or continue/fallback logic as appropriate
    //             }
    //         }
    //     }
    //     TestReference::SavedHolon(key) => match test_state.get_created_holon_by_key(&key) {
    //         Some(holon) => holon,
    //         None => {
    //             panic!("Holon with key {key} not found in created_holons");
    //         }
    //     },
    // };

    // 3. Build the DanceRequest
    let request =
        build_stage_new_from_clone_dance_request(original_holon_reference.clone(), new_key)
            .expect("Failed to build stage_new_from_clone request");

    debug!("Dance Request: {:#?}", request);

    // 4. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 5. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_from_clone request returned unexpected status: {}",
        response.description
    );

    // 6. If successful, verify the cloned Holon
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::HolonReference(cloned_holon) = response.body {
            info!("Cloned holon reference returned: {:?}", cloned_holon);

            assert_eq!(
                original_holon_reference.essential_content(context), // Phase 2 failing here
                cloned_holon.essential_content(context),
                "Cloned Holon content did not match original"
            );

            info!("Success! Cloned holon matched expected content");
        } else {
            panic!("Expected StagedRef in response body, but got {:?}", response.body);
        }
    }
}
