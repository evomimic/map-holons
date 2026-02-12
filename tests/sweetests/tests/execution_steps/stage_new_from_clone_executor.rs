use base_types::MapString;
use holon_dance_builders::stage_new_from_clone_dance::build_stage_new_from_clone_dance_request;
use holons_core::{dances::{ResponseBody, ResponseStatusCode}, HolonReference, HolonsContextBehavior};
use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};
use tracing::info;

/// Execute the StageNewFromClone step:
///  1) LOOKUP: convert the fixture `TestReference` into a runtime `HolonReference`
///  2) BUILD: construct the dance request with the source and `new_key`
///  3) CALL: perform the dance via the context-owned service
///  4) ASSERT: check `expected_status` and essential-content equality
///  5) RECORD: store the realized `StagedReference` in `ExecutionHolons` for downstream steps
pub async fn execute_stage_new_from_clone(
    state: &mut TestExecutionState,
    source_token: TestReference,
    new_key: MapString,
    expected_status: ResponseStatusCode,
    description: Option<String>,
) {
    let context = state.context();
    let description = match description {
        Some(dsc) => dsc,
        None => "Staging New Holon From Clone".to_string(),
    };
    info!("--- TEST STEP: {description} ---");


    // 1. LOOKUP — get the input handle for the clone source
    //    (enforces Saved ≙ Staged(Committed(LocalId)); no nursery fallback)
    let source_reference: HolonReference =
        state.resolve_source_reference(&context, &source_token).unwrap();

    // 2. BUILD — dance request to stage a new holon cloned from `source_reference`
    let request = build_stage_new_from_clone_dance_request(source_reference, new_key)
        .expect("Failed to build stage_new_from_clone request");

    // 3. CALL — use the context-owned call service
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request)
.await;

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "stage_new_from_clone request failed: {}",
        response.description
    );
    info!("Success! stage_new_from_clone DanceResponse matched expected");

    if expected_status == ResponseStatusCode::OK {
        // 5. ASSERT — on success, the body must be a HolonReference
        let response_holon_reference = match response.body {
            ResponseBody::HolonReference(hr) => hr,
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
        execution_reference.assert_essential_content_eq()
;

        // 6. RECORD — make execution result available for downstream steps
        state.record(&source_token, execution_reference).unwrap();
    }
}
