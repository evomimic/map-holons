use base_types::MapString;
use holons_core::HolonReference;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};
use tracing::info;

/// This function tests the ability to stage a new holon as a clone from an existing staged or saved holon.
/// It calls the `stage_new_from_clone` API.
///
pub async fn execute_stage_new_from_clone(
    state: &mut TestExecutionState,
    step_token: TestReference,
    new_key: MapString,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the clone source
    //    (enforces Saved ≙ Staged(Committed(LocalId)); no nursery fallback)
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt API call, confirm successful result
            context
                .mutation()
                .stage_new_from_clone(source_reference, new_key)
                .map_or_else(
                |e| panic!("Expected stage_new_from_clone to be successful, got {:?}", e),
                |staged_reference| {
                    info!("Success! stage_new_from_clone succeded as expected.");
                    // 3. ASSERT — essential content matches expected
                    let execution_handle =
                        ExecutionHandle::from(HolonReference::from(staged_reference));
                    let execution_reference =
                        ExecutionReference::from_token_execution(&step_token, execution_handle);
                    execution_reference.assert_essential_content_eq();
                    info!("Success! Holon's essential content matched expected");

                    // 4. RECORD — make this execution result available downstream
                    state.record(&step_token, execution_reference).unwrap();
                },
            );
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt API call, panic if the result does not match expected.
            let result = context
                .mutation()
                .stage_new_from_clone(source_reference, new_key);
            match result {
                Ok(_) => {
                    panic!("Expected stage_new_from_clone to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected stage_new_from_clone to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! stage_new_from_clone failed as expected.");
                    }
                }
            }
        }
    }
}
