use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::{debug, info};

/// Stages a clone of an existing holon via `TransactionAction::StageNewFromClone`.
pub async fn execute_stage_new_from_clone(
    state: &mut TestExecutionState,
    step_token: TestReference,
    new_key: MapString,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // Rebind SmartReferences to the current context to avoid cross-transaction errors.
    // SmartReferences from a prior commit carry the old transaction's context handle.
    let rebound_reference = match &source_reference {
        HolonReference::Smart(smart_ref) => {
            let context_handle = TransactionContextHandle::new(context.clone());
            HolonReference::Smart(SmartReference::new_from_id(context_handle, smart_ref.holon_id()))
        }
        other => other.clone(),
    };

    // 2. BUILD + DISPATCH
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::StageNewFromClone { original: rebound_reference, new_key },
    });
    let result = state.dispatch_command(command, "stage_new_from_clone").await;
    debug!("stage_new_from_clone result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Staged(staged_ref))) => {
            assert!(
                expected_error.is_none(),
                "stage_new_from_clone succeeded but expected {:?}",
                expected_error,
            );

            let holon_ref = HolonReference::Staged(staged_ref);
            let execution_handle = ExecutionHandle::from(holon_ref);
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Cloned holon's essential content matched expected");
            state.record(&step_token, execution_reference).unwrap();
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(
                Some(actual),
                expected_error,
                "stage_new_from_clone: unexpected error {:?}",
                e,
            );
        }
        Ok(other) => {
            panic!("stage_new_from_clone: expected Staged reference, got {:?}", other)
        }
    }
}
