use holons_prelude::prelude::*;
use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestReference};
use tracing::info;

/// Resolves a holon committed outside the fixture's ledger (e.g. a
/// schema-loaded descriptor) by key and records its `SmartReference` against
/// the step token so later steps can use it (typically as a relationship
/// target).
///
/// The lookup runs in the **live** transaction context (`state.context()`),
/// not an assertion context, so the recorded reference belongs to the
/// transaction in which it will be used.
///
/// Recording validates the token's key-only stub expectation via
/// `assert_expected_content_eq`, which matches saved-lookup stubs by key.
pub async fn execute_lookup_saved_holon_by_key(
    state: &mut TestExecutionState,
    step_token: TestReference,
    key: MapString,
    expected_error: Option<HolonErrorKind>,
) {
    info!("--- TEST STEP: Lookup saved holon by key '{}' ---", key.0);

    let context = state.context();

    match context.lookup().get_saved_holon_by_key(&key) {
        Ok(reference) => {
            let holon_reference = HolonReference::Smart(reference);
            assert!(
                expected_error.is_none(),
                "lookup_saved_holon_by_key: expected failure {:?} but found saved holon with key '{}'",
                expected_error,
                key.0
            );
            if !matches!(holon_reference, HolonReference::Smart(_)) {
                panic!(
                    "lookup_saved_holon_by_key: expected Smart reference for key '{}', got {:?}",
                    key.0, holon_reference
                );
            }

            let execution_reference = ExecutionReference::from_token_execution(
                &step_token,
                ExecutionHandle::from(holon_reference),
            );
            // Stub expectations are matched key-only; this validates the
            // resolved holon's key against the fixture-declared key.
            execution_reference.assert_expected_content_eq();
            state.record(&step_token, execution_reference).unwrap();
            info!("Success! lookup_saved_holon_by_key resolved key '{}'", key.0);
        }
        Err(error) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected_error,
                "lookup_saved_holon_by_key: unexpected error {:?}",
                error
            );
            info!("Success! lookup_saved_holon_by_key failed as expected for key '{}'", key.0);
        }
    }
}
