use async_std::task;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{debug, error, info, warn};

use rstest::*;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::mock_conductor::MockConductorConfig;
use crate::shared_test::test_data_types::{DanceTestExecutionState, DancesTestCase};

use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{HolonError, HolonId};
use holon_dance_builders::abandon_staged_changes_dance::build_abandon_staged_changes_dance_request;
use holons_core::{
    core_shared_objects::holon::state::AccessType,
    dances::{
        dance_response::{ResponseBody, ResponseStatusCode},
        DanceResponse,
    },
};
use holons_core::{StagedReference, WriteableHolon};
use integrity_core_types::{HolonNode, PropertyMap, PropertyName};
use rstest::*;

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
}
