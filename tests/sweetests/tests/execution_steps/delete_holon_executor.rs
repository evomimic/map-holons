use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};
use std::mem::discriminant;
use tracing::{
    // debug,
    info,
};

/// This executor tests the ability to delete a saved holon.
/// It calls the `delete_holon` API.
/// An execution holon is recorded regardless of whether the ExpectedTestResult is a success or failure.
///
pub async fn execute_delete_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    let HolonId::Local(local_id) = source_reference.holon_id().expect("Failed to get HolonId")
    else {
        panic!("Expected LocalId");
    };

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt delete, confirm successful result
            let result = delete_holon(&context, local_id);
            if let Err(e) = result {
                panic!("Expected delete_holon to be successful, got {:?}", e);
            } else {
                info!("Success! delete_holon succeded as expected.");
            }

            let execution_handle = ExecutionHandle::Deleted;
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);

            // 3. RECORD — make this execution result available downstream
            state.record(&step_token, execution_reference).unwrap();
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt delete, panic if the result does not match expected.
            delete_holon(&context, local_id).map_or_else(
                |e| {
                    // Compare only variant type, ignore inner string
                    if discriminant(&e) != discriminant(&expected_error) {
                        panic!(
                            "Expected delete_holon to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    }
                    let execution_handle = ExecutionHandle::from(source_reference);
                    let execution_reference =
                        ExecutionReference::from_token_execution(&step_token, execution_handle);

                    // 3. RECORD — make this execution result available downstream
                    state.record(&step_token, execution_reference).unwrap();
                    info!("Success! delete_holon failed as expected.");
                },
                |_| panic!("Expected delete_holon to error: {:?}, got Ok", expected_error),
            );
        }
    }
}
