use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, error, info, warn};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{DanceTestExecutionState, DancesTestCase};

use holons_core::{
    core_shared_objects::holon::state::AccessType,
    dances::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode},
    HolonError, HolonWritable, StagedReference,
};

use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::HolonId;
use integrity_core_types::{HolonNode, PropertyMap, PropertyName};

use holon_dance_builders::abandon_staged_changes_dance::build_abandon_staged_changes_dance_request;

/// This function builds and dances an `abandon_staged_changes` DanceRequest,
/// If the `ResponseStatusCode` returned by the dance != `expected_response`, panic to fail the test
/// Otherwise, if the dance returns an `OK` response,
///     confirm the Holon is in an `Abandoned` state and attempt various operations
///     that should be `NotAccessible` for holons an `Abandoned` state. If any of them do NOT
///     return a `NotAccessible` error, then panic to fail the test
/// Log a `info` level message marking the test step as Successful and return
///
pub async fn execute_abandon_staged_changes(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    staged_reference: StagedReference,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Abandon Staged Changes ---");

    // 1. Get the context from test_state
    let context = test_state.context();

    // 2. Build the request (state is handled inside dance_call)
    let request = build_abandon_staged_changes_dance_request(staged_reference.clone())
        .expect("Failed to build abandon_staged_changes request");

    info!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request).await;

    // 4. Validate response status
    assert_eq!(response.status_code, expected_response);
    info!("Dance returned {:?}", response.status_code);

    // 5. If successful, validate that operations on the abandoned Holon fail as expected
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::StagedRef(abandoned_holon) = &response.body {
            assert_eq!(
                abandoned_holon.with_property_value(
                    context, // Pass context for proper behavior
                    PropertyName(MapString("some_name".to_string())),
                    Some(BaseValue::BooleanValue(MapBoolean(true)))
                ),
                Err(HolonError::NotAccessible(
                    format!("{:?}", AccessType::Write),
                    "Immutable StagedHolon".to_string()
                ))
            );
            debug!("Confirmed abandoned holon is NotAccessible for `with_property_value`");
        } else {
            panic!("Expected abandon_staged_changes to return a StagedRef response, but it didn't");
        }
    }
    // warn!("\n\n--- TEST STEP: Abandon Staged Changes IS NOT CURRENTLY IMPLEMENTED---");
    // let request = build_abandon_staged_changes_dance_request(staged_reference.clone());
    //
    // info!("Dance Request: {:#?}", request);
    //
    // match request {
    //     Ok(valid_request) => {
    //         let response: DanceResponse =
    //             conductor.call(&cell.zome("dances"), "dance", valid_request).await;
    //         test_state.session_state = response.state.clone();
    //
    //         assert_eq!(response.status_code, expected_response);
    //
    //         info!("As expected, Dance returned {:?}", response.status_code);
    //
    //         match response.status_code {
    //             ResponseStatusCode::OK => {
    //                 // Dance response was OK, confirm that operations disallowed for Holons in an
    //                 // Abandoned state return NotAccessible error.
    //                 if let ResponseBody::StagedRef(abandoned_holon) = response.body.clone() {
    //                     match test_state
    //                         .session_state
    //                         .get_staging_area_mut()
    //                         .get_holon_mut(staged_index)
    //                     {
    //                         Ok(abandoned_holon) => {
    //                             // NOTE: We changed the access policies to ALLOW read access to
    //                             // Abandoned holons, so disabling the following two checks
    //                             // TODO: Consider adding checks that these operations are ALLOWED
    //                             // assert!(matches!(
    //                             //     abandoned_holon.get_property_value(
    //                             //         &PropertyName(MapString("some_name".to_string()))
    //                             //     ),
    //                             //     Err(HolonError::NotAccessible(_, _))
    //                             // ));
    //                             // debug!("Confirmed abandoned holon is NotAccessible for `get_property_value`");
    //                             //
    //                             // assert!(matches!(
    //                             //     abandoned_holon.get_key(),
    //                             //     Err(HolonError::NotAccessible(_, _))
    //                             // ));
    //                             // debug!("Confirmed abandoned holon is NotAccessible for `get_key`");
    //
    //                             assert!(matches!(
    //                                 abandoned_holon.with_property_value(
    //                                     PropertyName(MapString("some_name".to_string())),
    //                                     BaseValue::BooleanValue(MapBoolean(true))
    //                                 ),
    //                                 Err(HolonError::NotAccessible(_, _))
    //                             ));
    //                             debug!("Confirmed abandoned holon is NotAccessible for `with_property_value`");
    //
    //                             // TODO: support get_all_related_holons required for this assertion
    //                             // assert!(matches!(
    //                             //     abandoned_holon.get_related_holons(None),
    //                             //     Err(HolonError::NotAccessible(_, _))
    //                             // ));
    //                             // debug!("Confirmed abandoned holon is NotAccessible for `get_related_holons`");
    //                         }
    //                         Err(e) => {
    //                             panic!("Failed to get holon: {:?}", e);
    //                         }
    //                     }
    //                 } else {
    //                     panic!("Expected abandon_staged_changes to return an Index response, but it didn't");
    //                 }
    //             }
    //             _ => (),
    //         }
    //     }
    //     Err(error) => {
    //         panic!("{:?} Unable to build a abandon_staged_changes request ", error);
    //     }
    // }
}
