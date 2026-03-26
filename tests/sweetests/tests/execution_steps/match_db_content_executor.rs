use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestHolonState};
use tracing::info;

use holons_prelude::prelude::*;

/// Iterates through recorded saved holons and compares each saved execution
/// reference directly against its expected snapshot.
///
/// SmartReferences from the commit executor carry the committed transaction's context
/// handle, which may have stale cache state. We rebind them to a fresh assertion
/// context so that `all_related_holons()` can fetch relationship data from the DHT.

pub async fn execute_match_db_content(state: &mut TestExecutionState) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    let context = state
        .open_assertion_context("match_db_content")
        .await
        .expect("failed to open assertion transaction for match_db_content");

    for (id, resolved_reference) in state.holons().by_snapshot_id.clone() {
        if resolved_reference.expected_snapshot.state() == TestHolonState::Saved {
            let holon_reference = resolved_reference
                .execution_handle
                .get_holon_reference()
                .expect("HolonReference must be live for saved snapshots");
            if !matches!(holon_reference, HolonReference::Smart(_)) {
                panic!(
                    "Expected execution_reference for id: {:?} to be Smart, but got {:?}",
                    id, resolved_reference.execution_handle
                );
            }

            // Rebind SmartReference to assertion context for fresh cache/DHT access
            let rebound_reference = match &holon_reference {
                HolonReference::Smart(smart_ref) => {
                    let context_handle = TransactionContextHandle::new(context.clone());
                    HolonReference::Smart(SmartReference::new_from_id(
                        context_handle,
                        smart_ref.holon_id(),
                    ))
                }
                other => other.clone(),
            };

            let rebound_exec_ref = ExecutionReference {
                expected_snapshot: resolved_reference.expected_snapshot.clone(),
                execution_handle: ExecutionHandle::from(rebound_reference.clone()),
            };
            rebound_exec_ref.assert_essential_content_eq();
            info!(
                "SUCCESS! DB fetched holon matched expected for: \n {:?}",
                rebound_reference.summarize()
            );
        }
    }
}
