use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestHolonState};
use tracing::info;

use holons_prelude::prelude::*;

/// Iterates through recorded saved holons and compares each saved execution
/// reference directly against its expected snapshot.
///
/// SmartReferences remain bound to their space, so they continue to resolve saved
/// holons and relationships after the committing transaction has closed.

pub async fn execute_match_db_content(state: &mut TestExecutionState) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    for (id, resolved_reference) in state.holons().by_snapshot_id.clone() {
        // Only `Saved` snapshots carry full expected content. `SavedLookup` stubs
        // (holons saved outside the fixture, e.g. by schema loads) are intentionally
        // excluded here; they are matched key-only when reached as relationship
        // targets during graph comparison.
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

            let rebound_exec_ref = ExecutionReference {
                expected_snapshot: resolved_reference.expected_snapshot.clone(),
                execution_handle: ExecutionHandle::from(holon_reference.clone()),
            };
            rebound_exec_ref.assert_saved_content_eq(state.holons(), state.fixture_head_index());
            info!(
                "SUCCESS! DB fetched holon matched expected for: \n {:?}",
                holon_reference.summarize()
            );
        }
    }
}
