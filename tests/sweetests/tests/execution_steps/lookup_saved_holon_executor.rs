use holons_prelude::prelude::*;
use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestReference};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
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
/// `assert_essential_content_eq`, which matches saved-lookup stubs by key.
pub async fn execute_lookup_saved_holon_by_key(
    state: &mut TestExecutionState,
    step_token: TestReference,
    key: MapString,
    expected_error: Option<HolonErrorKind>,
) {
    info!("--- TEST STEP: Lookup saved holon by key '{}' ---", key.0);

    let context = state.context();

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::GetAllHolons,
    });
    let result = state.dispatch_command(command, "lookup_saved_holon_by_key").await.unwrap_or_else(
        |error| panic!("lookup_saved_holon_by_key: get_all_holons failed: {error:?}"),
    );
    let holons = match result {
        MapResult::Collection(collection) => collection,
        other => panic!("lookup_saved_holon_by_key: expected Collection, got {other:?}"),
    };

    match holons.get_by_key(&key) {
        Ok(Some(holon_reference)) => {
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
            execution_reference.assert_essential_content_eq();
            state.record(&step_token, execution_reference).unwrap();
            info!("Success! lookup_saved_holon_by_key resolved key '{}'", key.0);
        }
        Ok(None) => {
            assert_eq!(
                Some(HolonErrorKind::HolonNotFound),
                expected_error,
                "lookup_saved_holon_by_key: no saved holon found for key '{}'",
                key.0
            );
            info!("Success! lookup_saved_holon_by_key failed as expected for key '{}'", key.0);
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(
                Some(actual),
                expected_error,
                "lookup_saved_holon_by_key: unexpected error {:?}",
                e
            );
        }
    }
}
