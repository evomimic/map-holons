use holons_test::{ResolvedTestReference, TestExecutionState, TestReference};
use tracing::{debug, info};

use holons_prelude::prelude::*;

// TODO: Remove this import, direct access to HolonState should not be allowed from test layer.
// The need for this will go away once Holon is removed from ResponseBody
use holons_core::core_shared_objects::ReadableHolonState;

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(
    state: &mut TestExecutionState,
    source_tokens: &mut Vec<TestReference>,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. BUILD - dance request to commit
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response status and ResponseBody type
    assert_eq!(
        response.status_code, expected_status,
        "commit request returned unexpected status: {}",
        response.description
    );
    info!("Success! commit DanceResponse matched expected");

    // The response body is now a HolonReference::Transient to a CommitResponseType holon.
    let commit_response_body_reference = match response.body {
        ResponseBody::HolonReference(HolonReference::Transient(ref commit_ref)) => {
            commit_ref.clone()
        }
        other => panic!("Unexpected ResponseBody for commit: {:?}", other),
    };

    // TODO: Once TypeDescriptors are enabled, we should also check the HolonType of the ResponseBody

    // 4. GET - committed holons from the HolonsCommitted relationship.
    let committed_references = commit_response_body_reference
        .related_holons(context, CoreRelationshipTypeName::HolonsCommitted)
        .expect("Failed to read HolonsCommitted relationship");

    let committed_refs_guard = committed_references.read().unwrap();
    let commit_count: MapInteger = committed_refs_guard.get_count();
    debug!("Discovered {:?} committed holons", commit_count.0);

    // 5. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    let committed_refs_guard =
        committed_references.read().expect("Failed to read committed holons");
    for resulting_reference in committed_refs_guard.get_members() {
        let resolved_reference = ResolvedTestReference::from_reference_parts(
            source_tokens.pop().expect("Expected source token, vec should not be empty"),
            resulting_reference.clone(),
        );

        state.record_resolved(resolved_reference);
    }

    // 6. Optional: log a summary
    info!("Commit complete: {} holons committed", committed_refs_guard.get_count().0);
}
