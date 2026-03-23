use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, TestExecutionState, TestReference,
};
use integrity_core_types::PropertyMap;
use tracing::{
    // debug,
    info,
};

/// This executor tests the ability to create a new holon with an optional key. 
/// It calls the `new_holon` API.
/// 
pub async fn execute_new_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    key: Option<MapString>,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt create, confirm successful result
            let mut transient_reference = context
                .mutation()
                .new_holon(key)
                .unwrap_or_else(|e| {
                    panic!("Expected new_holon to successfully create a TransientReference, got error: {:#?}", e)
                });

            for (name, value) in properties {
                transient_reference.with_property_value(name.clone(), value).unwrap_or_else(
                    |error| panic!("failed to set property {:?} on holon: {}", name, error),
                );
            }
            info!("Success! new_holon successfully created as expected.");

            // 2. ASSERT — essential content matches expected
            let execution_handle = ExecutionHandle::from(HolonReference::from(transient_reference));
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Holon's essential content matched expected");

            // 3. RECORD — make this execution result available downstream
            state.record(&step_token, execution_reference).unwrap();
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt create, panic if the result does not match expected.
            let result = context.mutation().new_holon(key);
            match result {
                Ok(_) => {
                    panic!("Expected new_holon to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected new_holon to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! new_holon failed as expected.");
                    }
                }
            }
        }
    }
}
