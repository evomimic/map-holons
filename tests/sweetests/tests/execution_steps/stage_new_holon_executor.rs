use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};
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
    info!("Success! stage_new_holon DanceResponse matched expected");

    if response.status_code == ResponseStatusCode::OK {
        // 5. ASSERT — on success, the body must be a HolonReference
        let response_holon_reference = match response.body {
            ResponseBody::HolonReference(ref hr) => hr.clone(),
            other => {
                panic!("expected ResponseBody::HolonReference, got {:?}", other);
            }
        };

        // Build execution handle from runtime result
        let execution_handle = ExecutionHandle::from(response_holon_reference);

        // Canonical construction: token + execution outcome
        let execution_reference =
            ExecutionReference::from_token_execution(&source_token, execution_handle);

        // Validate expected vs execution-time content
        execution_reference.assert_essential_content_eq(context);
        info!("Success! Staged holon's essential content matched expected");

        // 6. RECORD — make execution result available for downstream steps
        state.record(&source_token, execution_reference).unwrap();
    }
}
