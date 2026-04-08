use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::{debug, info};

/// Deletes a staged holon via `TransactionAction::DeleteHolon`.
///
/// On success, records `ExecutionHandle::Deleted` so downstream steps treat this
/// token as deleted. No follow-up verification dance is performed — the success
/// of `DeleteHolon` is sufficient.
pub async fn execute_delete_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token to extract LocalId
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    let HolonId::Local(local_id) = source_reference.holon_id().expect("Failed to get HolonId")
    else {
        panic!("Expected LocalId for delete");
    };

    // 2. BUILD + DISPATCH
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::DeleteHolon { local_id },
    });
    let result = state.dispatch_command(command, "delete_holon").await;
    debug!("delete_holon result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::None) => {
            assert!(
                expected_error.is_none(),
                "delete_holon succeeded but expected {:?}",
                expected_error,
            );
            info!("Success! Holon deleted");
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "delete_holon: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("delete_holon: expected None, got {:?}", other),
    }

    // 4. RECORD
    let execution_handle = if expected_error.is_none() {
        ExecutionHandle::Deleted
    } else {
        ExecutionHandle::from(source_reference)
    };
    let execution_reference =
        ExecutionReference::from_token_execution(&step_token, execution_handle);
    state.record(&step_token, execution_reference).unwrap();
}
