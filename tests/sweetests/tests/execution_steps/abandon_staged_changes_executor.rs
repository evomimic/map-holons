use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holons_test::{ResolvedTestReference, ResultingReference, TestExecutionState, TestReference};

/// This function builds and dances an `abandon_staged_changes` DanceRequest,
/// If the `ResponseStatusCode` returned by the dance != `expected_status`, panic to fail the test
/// Otherwise, if the dance returns an `OK` response,
///     confirm the Holon is in an `Abandoned` state and attempt various operations
///     that should be `NotAccessible` for holons an `Abandoned` state. If any of them do NOT
///     return a `NotAccessible` error, then panic to fail the test
/// Log a `info` level message marking the test step as Successful and return
///
pub async fn execute_abandon_staged_changes(
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Abandon Staged Changes ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();

    // 2. BUILD — dance request to abandon holon
    let request = build_abandon_staged_changes_dance_request(source_reference)
        .expect("Failed to build abandon_staged_changes request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL — use the context-owned call service
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "abandon_staged_changes request returned unexpected status: {}",
        response.description
    );
    info!("Success! abandon_staged_changes DanceResponse matched expected");

    // 5. ASSERT — on success, the body should be a HolonReference to the abandoned holon.
    //            Compare essential content
    let response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("expected ResponseBody::HolonReference, got {:?}", other);
        }
    };
    let resulting_reference = ResultingReference::from(response_holon_reference);
    let resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
    resolved_reference.assert_essential_content_eq(context).unwrap();
    // Confirm that operations on the abandoned Holon fail as expected
    if let ResponseBody::HolonReference(mut abandoned_holon) = response.body {
        assert_eq!(
            abandoned_holon.with_property_value(
                context, // Pass context for proper behavior
                PropertyName(MapString("some_name".to_string())),
                BaseValue::BooleanValue(MapBoolean(true))
            ),
            Err(HolonError::NotAccessible(
                format!("{:?}", AccessType::Write),
                "Immutable".to_string()
            ))
        );
        debug!("Confirmed abandoned holon is NotAccessible for `with_property_value`");
    } else {
        panic!("Expected abandon_staged_changes to return a StagedRef response, but it didn't");
    }

    // 6. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(resolved_reference);
}
