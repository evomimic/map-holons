use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, WritableHolonAction,
};

/// Dispatches descriptor-assisted initialization and checks the declared snapshot.
pub async fn execute_with_descriptor(
    state: &mut TestExecutionState,
    step_token: TestReference,
    descriptor: TestReference,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();
    let source =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    let targets = match state.resolve_relationship_targets(
        &context,
        &CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        &[descriptor],
    ) {
        Ok(targets) => targets,
        Err(error) => {
            assert_eq!(
                Some(HolonErrorKind::from(&error)),
                expected_error,
                "with_descriptor: unexpected target resolution error {error:?}"
            );
            return;
        }
    };
    let descriptor = targets.into_iter().next().expect("one descriptor target");
    let result = state
        .dispatch_command(
            MapCommand::Holon(HolonCommand {
                context,
                target: source.clone(),
                action: HolonAction::Write(WritableHolonAction::WithDescriptor { descriptor }),
            }),
            "with_descriptor",
        )
        .await;
    match result {
        Ok(MapResult::None) => {
            assert!(
                expected_error.is_none(),
                "with_descriptor succeeded but expected {expected_error:?}"
            );
            let reference = ExecutionReference::from_token_execution(
                &step_token,
                ExecutionHandle::from(source),
            );
            reference.assert_expected_content_eq();
            state.record(&step_token, reference).unwrap();
        }
        Err(error) => assert_eq!(
            Some(HolonErrorKind::from(&error)),
            expected_error,
            "with_descriptor: unexpected error {error:?}"
        ),
        Ok(other) => panic!("with_descriptor: expected None, got {other:?}"),
    }
}
