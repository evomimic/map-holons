use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use pretty_assertions::assert_eq;
use tracing::info;

/// Abandons staged changes on a holon.
pub async fn execute_abandon_staged_changes(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    let mut staged_reference = match source_reference {
        HolonReference::Staged(staged_reference) => staged_reference,
        other => panic!("abandon_staged_changes: expected Staged reference, got {:?}", other),
    };

    // 2. APPLY — direct local mutation on the staged holon
    match staged_reference.abandon_staged_changes(&context) {
        Ok(()) => {
            assert!(expected_error.is_none(), "abandon_staged_changes expected failure but got OK",);
            info!("Success! abandon_staged_changes completed");

            let mut holon_reference = HolonReference::Staged(staged_reference);
            let execution_handle = ExecutionHandle::from(holon_reference.clone());
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle);
            execution_reference.assert_essential_content_eq();

            // Verify abandoned holon is immutable.
            assert_eq!(
                holon_reference.with_property_value(
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
    }
}
