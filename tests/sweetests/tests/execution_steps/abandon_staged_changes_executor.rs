use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestReference, ResolveBy};

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
    step_token: TestReference,
    expected_status: ResponseStatusCode,
    description: String,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. BUILD — dance request to abandon holon
    let request = build_abandon_staged_changes_dance_request(source_reference)
        .expect("Failed to build abandon_staged_changes request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL — use the context-owned call service
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request).await;

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "abandon_staged_changes request returned unexpected status: {}",
        response.description
    );
    info!("Success! abandon_staged_changes DanceResponse matched expected");

    if response.status_code == ResponseStatusCode::OK {
        let mut response_holon_reference = match response.body {
            ResponseBody::HolonReference(ref hr) => hr.clone(),
            other => panic!("expected ResponseBody::HolonReference, got {:?}", other),
        };

        let execution_handle = ExecutionHandle::from(response_holon_reference.clone());
        let execution_reference =
            ExecutionReference::from_token_execution(&step_token, execution_handle);

        execution_reference.assert_essential_content_eq();

        assert_eq!(
            response_holon_reference.with_property_value(
                PropertyName(MapString("some_name".to_string())),
                BaseValue::BooleanValue(MapBoolean(true))
            ),
            Err(HolonError::NotAccessible(
                format!("{:?}", AccessType::Write),
                "Immutable".to_string()
            ))
        );

        state.record(&step_token, execution_reference).unwrap();
    }
}
