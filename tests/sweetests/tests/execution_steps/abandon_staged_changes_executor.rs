use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

/// Abandons staged changes on a holon.
///
/// **Temporary dance fallback:** There is no native `TransactionAction::AbandonStagedChanges`
/// variant yet. This executor wraps the dance request inside `TransactionAction::Dance(...)`,
/// routing it through the Runtime while preserving the existing dance choreography.
///
/// TODO: Add a native `TransactionAction::AbandonStagedChanges` variant and migrate.
pub async fn execute_abandon_staged_changes(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. BUILD — wrap existing dance request in TransactionAction::Dance
    let dance_request = build_abandon_staged_changes_dance_request(source_reference)
        .expect("Failed to build abandon_staged_changes request");
    debug!("Dance Request (via TransactionAction::Dance): {:#?}", dance_request);

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::Dance(dance_request),
    });

    // 3. DISPATCH
    let result = state.dispatch_command(command, "abandon_staged_changes").await;
    debug!("abandon_staged_changes result: {:?}", &result);

    // 4. VALIDATE — extract DanceResponse from MapResult
    match result {
        Ok(MapResult::DanceResponse(response)) => {
            if expected_error.is_none() {
                assert_eq!(
                    response.status_code,
                    ResponseStatusCode::OK,
                    "abandon_staged_changes: unexpected status: {}",
                    response.description,
                );
            } else {
                assert_ne!(
                    response.status_code,
                    ResponseStatusCode::OK,
                    "abandon_staged_changes expected failure but got OK",
                );
                return;
            }
            info!("Success! abandon_staged_changes completed");

            let mut response_holon_reference = match response.body {
                ResponseBody::HolonReference(ref hr) => hr.clone(),
                other => panic!("expected ResponseBody::HolonReference, got {:?}", other),
            };

            let execution_handle = ExecutionHandle::from(response_holon_reference.clone());
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();

            // Verify abandoned holon is immutable
            assert_eq!(
                response_holon_reference.with_property_value(
                    PropertyName(MapString("some_name".to_string())),
                    BaseValue::BooleanValue(MapBoolean(true))
                ),
                Err(HolonError::NotAccessible(
                    format!("{:?}", AccessType::Write),
                    "Immutable".to_string()
                ))
            );

            state.record(&step_token, execution_reference).unwrap();
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(
                Some(actual),
                expected_error,
                "abandon_staged_changes: unexpected error {:?}",
                e,
            );
        }
        Ok(other) => panic!("abandon_staged_changes: expected DanceResponse, got {:?}", other),
    }
}
