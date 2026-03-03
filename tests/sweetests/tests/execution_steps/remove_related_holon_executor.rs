use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};

use tracing::{
    // debug,
    info,
};

use holons_prelude::prelude::*;

/// This function tests the ability to remove holons from a specified relationship_name.
/// It calls the 'remove_related_holons' mutation for the supplied HolonReference.
///
pub async fn execute_remove_related_holons(
    state: &mut TestExecutionState,
    step_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let mut source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    let holons_to_remove: Vec<HolonReference> =
        state.resolve_execution_references(&context, ResolveBy::Expected, &holons).unwrap();

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt mutation, confirm successful result
            let result =
                source_reference.remove_related_holons(relationship_name, holons_to_remove);

            if let Err(e) = result {
                panic!("Expected successful remove_related_holons mutation, got {:?}", e);
            }
            // Proceed with these steps only if a successful result is expected and achieved
            else {
                info!(
                    "Success! remove_related_holons mutation on source_reference succeeded as expected."
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
            // Attempt mutation, panic if the result does not match expected.
            let result =
                source_reference.remove_related_holons(relationship_name, holons_to_remove);
            match result {
                Ok(_) => {
                    panic!("Expected remove_related_holons to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected remove_related_holons to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! remove_related_holons failed as expected.");
                    }
                }
            }
        }
    }
}
