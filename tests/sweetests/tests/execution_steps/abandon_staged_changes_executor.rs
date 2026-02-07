use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};

/// This function builds and dances an `abandon_staged_changes` DanceRequest.
/// If the `ResponseStatusCode` returned by the dance != `expected_status`, panic to fail the test.
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
        state.resolve_source_reference(context, &source_token).unwrap();

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

    if response.status_code == ResponseStatusCode::OK {
        // 5. ASSERT — on success, the body should be a HolonReference to the abandoned holon.
        let mut response_holon_reference = match response.body {
            ResponseBody::HolonReference(ref hr) => hr.clone(),
            other => {
                panic!("expected ResponseBody::HolonReference, got {:?}", other);
            }
        };

        // Build execution handle from the runtime result
        let execution_handle = ExecutionHandle::from(response_holon_reference.clone());

        // Canonical construction: token + execution outcome
        let execution_reference = ExecutionReference::from_token_execution(
            &source_token,
            execution_handle,
        );

        // Validate expected vs execution-time content
        execution_reference
            .assert_essential_content_eq(context);

        // Confirm that operations on the abandoned Holon fail as expected
        assert_eq!(
            response_holon_reference.with_property_value(
                context,
                PropertyName(MapString("some_name".to_string())),
                BaseValue::BooleanValue(MapBoolean(true))
            ),
            Err(HolonError::NotAccessible(
                format!("{:?}", AccessType::Write),
                "Immutable".to_string()
            ))
        );
        debug!("Confirmed abandoned holon is NotAccessible for `with_property_value`");

        // 6. RECORD — make this execution result available for downstream steps
        state.record(&source_token, execution_reference).unwrap();
    }
}
