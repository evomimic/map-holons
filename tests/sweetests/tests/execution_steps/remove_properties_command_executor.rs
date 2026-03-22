use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};
use tracing::{
    // debug,
    info,
};

/// This executor tests the ability to remove properties from a holon.
/// It calls the `remove_properties` mutation for the supplied HolonReference.
///
pub async fn execute_remove_properties(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let mut source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt mutation, capturing the first error (if any)
            let remove_properties_result = properties.into_iter().try_for_each(|(name, _value)| {
                source_reference.remove_property_value(&name).map(|_| ()) // discard &mut Self, keep only success/failure
            });
            if let Err(e) = remove_properties_result {
                panic!("Expected successful remove_properties mutation, got {:?}", e);
            }
            // Proceed with these steps only if a successful result is expected and achieved
            else {
                info!(
                    "Success! remove_properties mutation on source_reference succeeded as expected."
                );
                // 3. ASSERT — essential content matches expected
                let execution_handle = ExecutionHandle::from(source_reference.clone());
                let execution_reference =
                    ExecutionReference::from_token_execution(&step_token, execution_handle);

                execution_reference.assert_essential_content_eq();
                info!("Success! Updated holon's essential content matched expected");

                // 4. RECORD — make this execution result available downstream
                state.record(&step_token, execution_reference).unwrap();
            }
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt mutation, panic if the first call does not match expected.
            for (name, value) in properties {
                let result = source_reference.with_property_value(name, value);
                match result {
                    Ok(_) => {
                        panic!("Expected remove_properties to error: {:?}, got Ok", expected_error)
                    }
                    Err(e) => {
                        if e != expected_error {
                            panic!(
                                "Expected remove_properties to error with: {:?}, but got {:?}",
                                expected_error, e
                            );
                        } else {
                            info!("Success! remove_properties failed as expected.");
                            // Loop should not continue as only the first result matters, any mismatch should have panicked.
                            break;
                        }
                    }
                }
            }
        }
    }
}
