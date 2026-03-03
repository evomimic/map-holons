use pretty_assertions::assert_eq;
use tracing::{
    // debug,
    info,
};

use holons_prelude::prelude::*;

use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};

/// This executor tests the ability to mark a staged holon as 'abandoned'.
/// It calls the `abandon_staged_changes` method on a StagedReference.
/// Then confirms the Holon is in an `Abandoned` state and attempts various operations
/// that should be `NotAccessible` for holons in an `Abandoned` state. 
pub async fn execute_abandon_staged_changes(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    let mut staged_reference = match source_reference {
        HolonReference::Staged(ref sr) => sr.clone(),
        _ => {
            panic!("Can only abandon staged holons... step token must resolve to a StagedReference")
        }
    };

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt the abandon, confirm successful result
            let result = staged_reference.abandon_staged_changes(&context);
            if let Err(e) = result {
                panic!("Expected abandon_staged_changes to be successful, got {:?}", e);
            }
            // Proceed with these steps only if a successful result is expected and achieved
            else {
                assert_eq!(
                    staged_reference.with_property_value(
                        PropertyName(MapString("some_name".to_string())),
                        BaseValue::BooleanValue(MapBoolean(true))
                    ),
                    Err(HolonError::NotAccessible(
                        format!("{:?}", AccessType::Write),
                        "Immutable".to_string()
                    ))
                );
                info!("Success! abandon_staged_changes succeded as expected.");
            }

            // 3. ASSERT — essential content matches expected
            let execution_handle = ExecutionHandle::from(source_reference);
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Holon's essential content matched expected");

            // 4. RECORD — make this execution result available downstream
            state.record(&step_token, execution_reference).unwrap();
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt the abandon, panic if the result does not match expected.
            staged_reference.abandon_staged_changes(&context).map_or_else(
                |e| {
                    if e != expected_error {
                        panic!(
                            "Expected abandon_staged_changes to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    }
                    info!("Success! abandon_staged_changes failed as expected.");
                },
                |_| {
                    panic!("Expected abandon_staged_changes to error: {:?}, got Ok", expected_error)
                },
            );
        }
    }
}
