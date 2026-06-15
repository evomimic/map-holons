use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedCommitStatus, TestExecutionState, TestReference,
};
use integrity_core_types::HolonErrorKind;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use std::collections::BTreeMap;
use tracing::{debug, info, trace};

use holons_prelude::prelude::*;

/// Dispatches a `Commit` command through the Runtime and validates the result.
///
/// Asserts the `CommitRequestStatus` on the commit response against
/// `expected_status`. An `Incomplete` commit is still an `Ok` response: Pass-1
/// holons are saved while a Pass-2 (relationship persistence) failure is
/// recorded on the response, so saved-holon registration proceeds for both
/// statuses.
///
/// On success, reads committed holons from the `SavedHolons` relationship on the
/// commit response holon and registers them in the test execution state.
pub async fn execute_commit(
    state: &mut TestExecutionState,
    expected_tokens: Vec<TestReference>,
    expected_status: ExpectedCommitStatus,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();

    // 1. BUILD — transaction commit command
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::Commit,
    });

    // 2. DISPATCH
    let result = state.dispatch_command(command, "commit").await;
    debug!("Commit result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Transient(commit_response_ref))) => {
            assert!(expected_error.is_none(), "commit succeeded but expected {:?}", expected_error,);

            let actual_status = match commit_response_ref
                .property_value(&CorePropertyTypeName::CommitRequestStatus)
            {
                Ok(Some(PropertyValue::StringValue(MapString(status)))) => status,
                Ok(other) => panic!(
                    "commit: expected string CommitRequestStatus on commit response, got {:?}",
                    other
                ),
                Err(e) => panic!("commit: failed to read CommitRequestStatus: {:?}", e),
            };
            assert_eq!(
                actual_status,
                expected_status.to_string(),
                "Expected CommitRequestStatus={}, got {}",
                expected_status,
                actual_status
            );
            info!("Success! Commit completed via Runtime dispatch with status {}", actual_status);

            // 4. GET — committed holons from the SavedHolons relationship
            let committed_references = commit_response_ref
                .related_holons(CoreRelationshipTypeName::SavedHolons)
                .expect("Failed to read SavedHolons relationship");

            let committed_refs_guard = committed_references.read().unwrap();
            let commit_count: MapInteger = committed_refs_guard.get_count();
            debug!("Discovered {:?} committed holons", commit_count.0);

            // 5. RECORD — register committed holons so tokens become resolvable
            let holon_collection =
                committed_references.read().expect("Failed to read committed holons");

            // Temporary key-based matching: source token (expected) → resulting reference (actual)
            // TODO: solve or migrate issue 352
            let mut index: usize = 0;
            let mut keyed_index = BTreeMap::new();
            for token in &expected_tokens {
                let key = token.expected_reference().clone().key().unwrap().expect(
                    "For these testing purposes, source token (TestReference) must have a key",
                );
                keyed_index.insert(key, index);
                index += 1;
            }
            for holon_reference in holon_collection.get_members() {
                let source_index = keyed_index
                    .get(
                        &holon_reference.key().unwrap().expect(
                            "For these testing purposes, resulting reference (HolonReference) must have a key",
                        ),
                    )
                    .expect("Expected source token to be indexed by key");
                let token = &expected_tokens[*source_index];
                let execution_handle = ExecutionHandle::from(holon_reference.clone());
                let execution_reference =
                    ExecutionReference::from_token_execution(token, execution_handle);
                state.record(token, execution_reference).unwrap();
            }

            trace!("Commit complete: {} holons committed", committed_refs_guard.get_count().0);
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "commit: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("commit: expected Transient reference, got {:?}", other),
    }
}
