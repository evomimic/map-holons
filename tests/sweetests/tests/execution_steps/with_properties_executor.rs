use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, WritableHolonAction,
};
use tracing::{debug, info};

/// Applies properties to a holon via `WritableHolonAction::WithPropertyValue` dispatched
/// through the Runtime. Each property is dispatched as a separate command.
pub async fn execute_with_properties(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. DISPATCH — one WithPropertyValue command per property
    for (name, value) in &properties {
        let command = MapCommand::Holon(HolonCommand {
            context: context.clone(),
            target: source_reference.clone(),
            action: HolonAction::Write(WritableHolonAction::WithPropertyValue {
                name: name.clone(),
                value: value.clone(),
            }),
        });
        let result = state.dispatch_command(command, "with_property_value").await;
        debug!("with_property_value({:?}) result: {:?}", name, &result);

        match result {
            Ok(MapResult::None) => {}
            Err(e) => {
                let actual = HolonErrorKind::from(&e);
                assert_eq!(
                    Some(actual),
                    expected_error,
                    "with_property_value: unexpected error {:?}",
                    e,
                );
                return; // error path — stop processing
            }
            Ok(other) => panic!("with_property_value: expected None, got {:?}", other),
        }
    }

    // If we expected an error but all properties succeeded
    assert!(
        expected_error.is_none(),
        "with_properties: all writes succeeded but expected {:?}",
        expected_error,
    );

    // 3. VALIDATE + RECORD — the source_reference reflects mutations in-place
    let execution_handle = ExecutionHandle::from(source_reference);
    let execution_reference =
        ExecutionReference::from_token_execution(&step_token, execution_handle);
    execution_reference.assert_essential_content_eq();
    info!("Success! Updated holon's essential content matched expected");
    state.record(&step_token, execution_reference).unwrap();
}
