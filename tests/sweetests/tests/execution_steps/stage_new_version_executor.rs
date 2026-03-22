use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedTestResult, ResolveBy, TestExecutionState,
    TestReference,
};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

/// This function tests the ability to stage a new version of a holon.
/// It calls the `stage_new_version` API.
///
pub async fn execute_stage_new_version(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_result: ExpectedTestResult,
    version_count: MapInteger,
    expected_duplicate_error: Option<ResponseStatusCode>,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    // Can only stage_new_version from SmartReference
    let smart_reference = match source_reference {
        HolonReference::Smart(ref sr) => sr.clone(),
        other => {
            panic!("{}", format!("expected lookup to return SmartReference, got {:?}", other));
        }
    };

    // 2. MATCH EXPECTED - confirm actual against expected result
    match expected_result {
        ExpectedTestResult::Success => {
            // Attempt API call, confirm successful result
            context
                .mutation()
                .stage_new_version(smart_reference.clone())
                .map_or_else(
                |e| panic!("Expected stage_new_version to be successful, got {:?}", e),
                |staged_reference| {
                    info!("Success! stage_new_version succeded as expected.");
                    // 3. ASSERT — essential content matches expected
                    let execution_handle =
                        ExecutionHandle::from(HolonReference::from(staged_reference.clone()));
                    let execution_reference =
                        ExecutionReference::from_token_execution(&step_token, execution_handle.clone());
                    execution_reference.assert_essential_content_eq();
                    info!("Success! Holon's essential content matched expected");

                    // 4. RECORD — make this execution result available downstream
                    state.record(&step_token, execution_reference).unwrap();

            // 5. Verify the new version has the original holon as its predecessor.
            let predecessor = staged_reference.predecessor().unwrap().unwrap();

            assert_eq!(
                predecessor.holon_id().unwrap(),
                smart_reference.holon_id(),
                "Predecessor relationship did not match expected"
            );

            // 6. Verify base-key staging behavior
            let original_holon_key = source_reference.key().unwrap().unwrap();
            let by_base = context.lookup().get_staged_holon_by_base_key(&original_holon_key);

            match by_base {
                Ok(staged_reference) => {
                    if let Some(_code) = &expected_duplicate_error {
                        panic!(
                            "{}",
                            format!(
                                "Expected get_staged_holon_by_base_key to return {:?}",
                                expected_duplicate_error
                            )
                        );
                    } else {
                        let holon_reference = execution_handle
                            .get_holon_reference()
                            .expect("HolonReference must be live");

                        assert_eq!(
                            HolonReference::Staged(staged_reference.clone()),
                            holon_reference,
                            "get_staged_holon_by_base_key did not match expected"
                        );

                        // 7. Verify versioned-key lookup
                        let by_version = context
                            .lookup()
                            .get_staged_holon_by_versioned_key(
                                &staged_reference.versioned_key().unwrap(),
                            )
                            .unwrap();

                        assert_eq!(
                            holon_reference,
                            HolonReference::Staged(by_version),
                            "get_staged_holon_by_versioned_key did not match expected"
                        );

                        info!("Success! New version Holon matched expected content and relationships.");
                    }
                }
                Err(e) => {
                    if let Some(expected_code) = &expected_duplicate_error {
                        let actual_code = ResponseStatusCode::from(e.clone());
                        assert_eq!(
                            actual_code, *expected_code,
                            "Unexpected error status from get_staged_holon_by_base_key: {:?}",
                            e
                        );

                        if *expected_code == ResponseStatusCode::Conflict {
                            assert!(
                        matches!(e, HolonError::DuplicateError(_, _)),
                        "Expected DuplicateError from get_staged_holon_by_base_key, got {:?}",
                        e
                    );
                        }

                        debug!(
                    "Confirmed get_staged_holon_by_base_key returned a duplicate error {:?}",
                    expected_duplicate_error
                );
                        // Confirm that get_staged_holons_by_base_key returns two staged references for the two versions.
                        let staged_references = context
                            .lookup()
                            .get_staged_holons_by_base_key(&original_holon_key)
                            .unwrap();
                        let length = staged_references.len();

                        if length != version_count.0 as usize {
                            panic!("{}", format!(
                        "get_staged_holons_by_base_key returned: {:?} staged references, expected {:?}",
                        length, version_count
                    ));
                        }
                        let first_reference_content =
                            staged_references[0].essential_content().unwrap();
                        let second_reference_content =
                            staged_references[1].essential_content().unwrap();

                        if first_reference_content != second_reference_content {
                            panic!("References returned by get_staged_holons_by_base_key do not match essential content");
                        }
                    } else {
                        panic!("Expected get_staged_holon_by_base_key to return OK");
                    }
                }
            }
        }
        );
        }
        ExpectedTestResult::Failure(expected_error) => {
            // Attempt API call, panic if the result does not match expected.
            let result = context.mutation().stage_new_version(smart_reference);
            match result {
                Ok(_) => {
                    panic!("Expected stage_new_version to error: {:?}, got Ok", expected_error)
                }
                Err(e) => {
                    if e != expected_error {
                        panic!(
                            "Expected stage_new_version to error with: {:?}, but got {:?}",
                            expected_error, e
                        );
                    } else {
                        info!("Success! stage_new_version failed as expected.");
                    }
                }
            }
        }
    }
}
