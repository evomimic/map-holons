use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::{debug, info};

/// Stages a transient holon into the nursery via `TransactionAction::StageNewHolon`.
pub async fn execute_stage_new_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token to a TransientReference
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    let transient_reference = match source_reference {
        HolonReference::Transient(tr) => tr,
        other => panic!("expected TransientReference, got {:?}", other),
    };

    // 2. BUILD + DISPATCH
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::StageNewHolon { source: transient_reference },
    });
    let result = state.dispatch_command(command, "stage_new_holon").await;
    debug!("stage_new_holon result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Staged(staged_ref))) => {
            assert!(
                expected_error.is_none(),
                "stage_new_holon succeeded but expected {:?}",
                expected_error,
            );

            let holon_ref = HolonReference::Staged(staged_ref);
            let execution_handle = ExecutionHandle::from(holon_ref);
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Staged holon's essential content matched expected");
            state.record(&step_token, execution_reference).unwrap();
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "stage_new_holon: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("stage_new_holon: expected Staged reference, got {:?}", other),
    }
}
