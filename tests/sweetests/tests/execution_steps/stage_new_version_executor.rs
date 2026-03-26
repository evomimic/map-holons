use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::{debug, info};

/// Stages a new version of an existing holon via `TransactionAction::StageNewVersionFromId`,
/// then verifies predecessor, base-key, and versioned-key lookups.
pub async fn execute_stage_new_version(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_error: Option<HolonErrorKind>,
    version_count: MapInteger,
    expected_staging_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. LOOKUP — resolve source token to get the original holon's id
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();
    let original_holon_id = source_reference.holon_id().expect("Failed to get HolonId");

    // 2. BUILD + DISPATCH — StageNewVersionFromId
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::StageNewVersionFromId { holon_id: original_holon_id.clone() },
    });
    let result = state.dispatch_command(command, "stage_new_version").await;
    debug!("stage_new_version result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Staged(staged_ref))) => {
            assert!(
                expected_error.is_none(),
                "stage_new_version succeeded but expected {:?}",
                expected_error,
            );

            let response_ref = HolonReference::Staged(staged_ref.clone());
            let execution_handle = ExecutionHandle::from(response_ref.clone());
            let execution_reference =
                ExecutionReference::from_token_execution(&step_token, execution_handle.clone());
            execution_reference.assert_essential_content_eq();
            info!("Success! Staged new version holon's essential content matched expected");

            // 4. RECORD
            state.record(&step_token, execution_reference).unwrap();

            // 5. Verify predecessor relationship (self-resolving read)
            let predecessor = response_ref.predecessor().unwrap().unwrap();
            assert_eq!(
                predecessor.holon_id().unwrap(),
                original_holon_id,
                "Predecessor relationship did not match expected"
            );

            // 6. Verify base-key staging behavior via TransactionAction lookups
            let original_holon_key = source_reference.key().unwrap().unwrap();

            let by_base_command = MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::GetStagedHolonByBaseKey {
                    key: original_holon_key.clone(),
                },
            });
            let by_base_result =
                state.dispatch_command(by_base_command, "get_staged_holon_by_base_key").await;

            match by_base_result {
                Ok(MapResult::Reference(HolonReference::Staged(base_staged_ref))) => {
                    if let Some(_kind) = &expected_staging_error {
                        panic!(
                            "Expected get_staged_holon_by_base_key to return {:?}",
                            expected_staging_error,
                        );
                    }

                    let holon_reference = execution_handle
                        .get_holon_reference()
                        .expect("HolonReference must be live");

                    assert_eq!(
                        HolonReference::Staged(base_staged_ref.clone()),
                        holon_reference,
                        "get_staged_holon_by_base_key did not match expected"
                    );

                    // 7. Verify versioned-key lookup
                    let by_version_command = MapCommand::Transaction(TransactionCommand {
                        context: context.clone(),
                        action: TransactionAction::GetStagedHolonByVersionedKey {
                            key: base_staged_ref.versioned_key().unwrap(),
                        },
                    });
                    let by_version_result = state
                        .dispatch_command(by_version_command, "get_staged_holon_by_versioned_key")
                        .await;
                    match by_version_result {
                        Ok(MapResult::Reference(HolonReference::Staged(version_ref))) => {
                            assert_eq!(
                                holon_reference,
                                HolonReference::Staged(version_ref),
                                "get_staged_holon_by_versioned_key did not match expected"
                            );
                        }
                        other => panic!(
                            "get_staged_holon_by_versioned_key: unexpected result {:?}",
                            other
                        ),
                    }

                    info!("Success! New version Holon matched expected content and relationships.");
                }
                Err(e) => {
                    if let Some(expected_kind) = &expected_staging_error {
                        let actual_kind = HolonErrorKind::from(&e);
                        assert_eq!(
                            actual_kind, *expected_kind,
                            "Unexpected error kind from get_staged_holon_by_base_key: {:?}",
                            e,
                        );

                        if *expected_kind == HolonErrorKind::DuplicateError {
                            assert!(
                                matches!(e, HolonError::DuplicateError(_, _)),
                                "Expected DuplicateError, got {:?}",
                                e,
                            );
                        }

                        debug!(
                            "Confirmed get_staged_holon_by_base_key returned {:?}",
                            expected_staging_error,
                        );

                        // Verify get_staged_holons_by_base_key returns expected count
                        let by_base_multi_command = MapCommand::Transaction(TransactionCommand {
                            context: context.clone(),
                            action: TransactionAction::GetStagedHolonsByBaseKey {
                                key: original_holon_key.clone(),
                            },
                        });
                        let by_base_multi_result = state
                            .dispatch_command(
                                by_base_multi_command,
                                "get_staged_holons_by_base_key",
                            )
                            .await;

                        match by_base_multi_result {
                            Ok(MapResult::References(refs)) => {
                                let length = refs.len();
                                assert_eq!(
                                    length,
                                    version_count.0 as usize,
                                    "get_staged_holons_by_base_key returned {} references, expected {}",
                                    length,
                                    version_count.0,
                                );
                                let first_content = refs[0].essential_content().unwrap();
                                let second_content = refs[1].essential_content().unwrap();
                                assert_eq!(
                                    first_content, second_content,
                                    "References from get_staged_holons_by_base_key do not match essential content"
                                );
                            }
                            other => panic!(
                                "get_staged_holons_by_base_key: unexpected result {:?}",
                                other
                            ),
                        }
                    } else {
                        panic!("Expected get_staged_holon_by_base_key to return OK, got {:?}", e);
                    }
                }
                Ok(other) => panic!(
                    "get_staged_holon_by_base_key: expected Staged reference, got {:?}",
                    other
                ),
            }
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "stage_new_version: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("stage_new_version: expected Staged reference, got {:?}", other),
    }
}
