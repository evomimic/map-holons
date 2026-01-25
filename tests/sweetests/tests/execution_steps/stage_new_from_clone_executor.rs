use base_types::MapString;
use core_types::HolonError;
use holon_dance_builders::stage_new_from_clone_dance::build_stage_new_from_clone_dance_request;
use holons_core::{
    dances::{ResponseBody, ResponseStatusCode},
    HolonReference, HolonsContextBehavior,
};
use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};

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
) {
    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the clone source
    //    (enforces Saved ≙ Staged(Committed(LocalId)); no nursery fallback)
    let source_reference: HolonReference =
        state.resolve_source_reference(context, &source_token).unwrap();

    // 2. BUILD — dance request to stage a new holon cloned from `source_reference`
    let request = build_stage_new_from_clone_dance_request(source_reference, new_key)
        .expect("Failed to build stage_new_from_clone request");

    // 3. CALL — use the context-owned call service
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    assert_eq!(response.status_code, expected_status);

    if expected_status == ResponseStatusCode::OK {
        // 4. ASSERT — on success, the body should be a HolonReference to the newly staged holon.
        //            Compare essential content (source vs. resolved) without durable fetch.
        let response_holon_reference = match response.body {
            ResponseBody::HolonReference(hr) => hr,
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

        // 5. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
        state.record(source_token.expected_id().unwrap(), resolved_reference);
    }
}
