use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function stages a new holon. It builds and dances a `stage_new_holon` DanceRequest for the
/// supplied Holon and confirms a Success response
///
pub async fn execute_stage_new_holon(
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Staging a new Holon via DANCE ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(context, &source_token).unwrap();

    // Can only stage Transient
    let transient_reference = match source_reference {
        HolonReference::Transient(tr) => tr,
        other => {
            panic!("{}", format!("expected lookup to return TransientReference, got {:?}", other));
        }
    };
    // 2. BUILD - the stage_new_holon DanceRequest
    let request = build_stage_new_holon_dance_request(transient_reference)
        .expect("Failed to build stage_new_holon request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "stage_new_holon request failed: {}",
        response.description
    );

    // 5. ASSERT the staged holon's content matches
    let response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let resulting_reference = ResultingReference::from(response_holon_reference);
    let resolved_reference = ExecutionReference::from_reference_parts(
        source_token.expected_snapshot(),
        resulting_reference,
    );
    resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Staged holon's essential content matched expected");

    // 6. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
    state.record(&source_token, resolved_reference).unwrap();
}
