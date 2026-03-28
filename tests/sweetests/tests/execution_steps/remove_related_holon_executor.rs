use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, WritableHolonAction,
};
use tracing::{debug, info};

/// Removes related holons from a target via `WritableHolonAction::RemoveRelatedHolons`
/// dispatched through the Runtime.
pub async fn execute_remove_related_holons(
    state: &mut TestExecutionState,
    step_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source and target holons
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    let holons_to_remove: Vec<HolonReference> =
        state.resolve_execution_references(&context, ResolveBy::Expected, &holons).unwrap();

    // 2. DISPATCH
    let command = MapCommand::Holon(HolonCommand {
        context: context.clone(),
        target: source_reference.clone(),
        action: HolonAction::Write(WritableHolonAction::RemoveRelatedHolons {
            name: relationship_name,
            holons: holons_to_remove,
        }),
    });
    let result = state.dispatch_command(command, "remove_related_holons").await;
    debug!("remove_related_holons result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::None) => {
            assert!(
                expected_error.is_none(),
                "remove_related_holons succeeded but expected {:?}",
                expected_error,
            );
            info!("Success! Related holons removed");

            // 4. RECORD — source_reference reflects mutation in-place
            let execution_handle = ExecutionHandle::from(source_reference);
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Updated holon's essential content matched expected");
            state.record(&step_token, execution_reference).unwrap();
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(
                Some(actual),
                expected_error,
                "remove_related_holons: unexpected error {:?}",
                e,
            );
        }
        Ok(other) => panic!("remove_related_holons: expected None, got {:?}", other),
    }
}
