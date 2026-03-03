use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};
use tracing::{
    // debug,
    info,
};

/// This function tests the ability to stage a new holon.
/// It calls the `stage_new_holon` API.
///
pub async fn execute_stage_new_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // Can only stage Transient
    let transient_reference = match source_reference {
        HolonReference::Transient(ref tr) => tr.clone(),
        other => {
            panic!("{}", format!("expected lookup to return TransientReference, got {:?}", other));
        }
    };
    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt the API call, confirm successful result
            stage_new_holon(&context, transient_reference).map_or_else(
                |e| panic!("Expected stage_new_holon to be successful, got {:?}", e),
                |staged_reference| {
                    info!("Success! stage_new_holon succeded as expected.");
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
            // Attempt the API call, panic if the result does not match expected.
            let result = stage_new_holon(&context, transient_reference);
            match result {
                Ok(_) => {
                    panic!("Expected stage_new_holon to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected stage_new_holon to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! stage_new_holon failed as expected.");
                    }
                }
            }
        }
    }
}
