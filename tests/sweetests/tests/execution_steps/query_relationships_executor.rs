use holons_test::{ExpectedTestResult, ResolveBy, TestExecutionState, TestReference};

use holons_prelude::prelude::*;
use tracing::{
    // debug,
    info,
};

// TODO: need to match on expected content

/// This executor tests the ability to fetch relationships for a holon.
/// It calls the `related_holons' getter for the supplied HolonReference.
/// 
pub async fn execute_query_relationships(
    state: &mut TestExecutionState,
    step_token: TestReference,
    query_expression: QueryExpression,
    expected_result: ExpectedTestResult,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt query, confirm successful result
            let result = source_reference.related_holons(&query_expression.relationship_name);

            if let Err(e) = result {
                panic!(
                    "Expected related_holons to successfully fetch relationships , got {:?}",
                    e
                );
            }
            // Proceed with these steps only if a successful result is expected and achieved
            else {
                info!(
                    "Success! related_holons fetched {:?} successfully as expected.",
                    query_expression
                );
            }
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt query, panic if the result does not match expected.
            let result = source_reference.related_holons(&query_expression.relationship_name);
            match result {
                Ok(_) => {
                    panic!("Expected related_holons to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected related_holons to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! related_holons failed as expected.");
                    }
                }
            }
        }
    }

    // TODO:  Match on response.body node collection expected vs actual
}
