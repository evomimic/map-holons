use holons_prelude::prelude::*;
use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestReference};
use integrity_core_types::PropertyMap;
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, TransactionAction, TransactionCommand,
    WritableHolonAction,
};
use tracing::{debug, info};

/// Creates a new transient holon via `TransactionAction::NewHolon`, applies properties
/// via `HolonCommand::Write(WithPropertyValue)`, validates, and records.
pub async fn execute_new_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    key: Option<MapString>,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. BUILD + DISPATCH — NewHolon command
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::NewHolon { key },
    });
    let result = state.dispatch_command(command, "new_holon").await;
    debug!("new_holon result: {:?}", &result);

    // 2. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Transient(transient_ref))) => {
            assert!(
                expected_error.is_none(),
                "new_holon succeeded but expected {:?}",
                expected_error,
            );

            // 3. Apply properties via HolonCommand::Write
            let holon_ref = HolonReference::Transient(transient_ref.clone());
            for (name, value) in properties {
                let prop_command = MapCommand::Holon(HolonCommand {
                    context: context.clone(),
                    target: holon_ref.clone(),
                    action: HolonAction::Write(WritableHolonAction::WithPropertyValue {
                        name: name.clone(),
                        value,
                    }),
                });
                state.runtime().execute_command(prop_command).await.unwrap_or_else(|e| {
                    panic!("failed to set property {:?} on new holon: {:?}", name, e)
                });
            }

            // 4. RECORD
            let execution_handle = ExecutionHandle::from(holon_ref);
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();
            info!("Success! Holon's essential content matched expected");
            state.record(&step_token, execution_reference).unwrap();
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "new_holon: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("new_holon: expected Transient reference, got {:?}", other),
    }
}
