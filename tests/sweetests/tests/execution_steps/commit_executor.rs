use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};
use std::collections::BTreeMap;
use tracing::{debug, info, trace};

use holons_prelude::prelude::*;

/// This function builds and dances a `commit` DanceRequest and confirms a Success response.
///
/// Source tokens are needed for this step in order to build a ExecutionReference.
pub async fn execute_commit(
    state: &mut TestExecutionState,
    expected_tokens: Vec<TestReference>, // list of expected tokens to resolve
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. BUILD - dance request to commit
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
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

    if response.status_code == ResponseStatusCode::OK {
        // 4. GET - committed holons from the HolonsCommitted relationship.
        let committed_references = commit_response_body_reference
            .related_holons(context, CoreRelationshipTypeName::SavedHolons)
            .expect("Failed to read HolonsCommitted relationship");

        let committed_refs_guard = committed_references.read().unwrap();
        let commit_count: MapInteger = committed_refs_guard.get_count();
        debug!("Discovered {:?} committed holons", commit_count.0);

        // 5. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
        let holon_collection =
            committed_references.read().expect("Failed to read committed holons");
        // Temporary 'key' workaround for matching source token (expected) to resulting reference (actual).
        // TODO: solve or migrate issue 352
        let mut index: usize = 0;
        let mut keyed_index = BTreeMap::new();
        for token in &expected_tokens {
            let key =
                token.expected_reference().clone().key(context).unwrap().expect(
                    "For these testing purposes, source token (TestReference) must have a key",
                );
            keyed_index.insert(key, index);
            index += 1;
        }
        for holon_reference in holon_collection.get_members() {
            let source_index = keyed_index.get(&holon_reference.key(context).unwrap().expect(
            "For these testing purposes, resulting reference (HolonReference) must have a key",
        )).expect("Something went wrong in this functions logic.. Expected source token to be indexed by key");
            let token = &expected_tokens[*source_index];
            let expected = token.expected_snapshot();
            let resolved_reference = ExecutionReference::from_reference_parts(
                expected.clone(),
                ResultingReference::from(holon_reference.clone()),
            );

            state.record(token, resolved_reference).unwrap();
        }

        // 6. Optional: log a summary
        trace!("Commit complete: {} holons committed", committed_refs_guard.get_count().0);
    }
}
