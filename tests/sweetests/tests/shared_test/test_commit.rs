use crate::mock_conductor::MockConductorConfig;
use async_std::task;
use holons_prelude::prelude::*;
use rstest::*;
use std::collections::BTreeMap;
use tracing::{debug, info, warn};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::{
    // mock_conductor::MockConductorConfig,
    test_data_types::{DanceTestExecutionState, DancesTestCase},
};
use holons_prelude::prelude::*;

// TODO: Remove this import, direct access to HolonState should not be allowed from test layer.
// The need for this will go away once Holon is removed from ResponseBody

use holons_core::core_shared_objects::ReadableHolonState;

/// This function builds and dances a `commit` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_commit(test_state: &mut DanceTestExecutionState) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    let ctx_arc = test_state.context(); // Arc lives until end of scope
    let context = ctx_arc.as_ref();

    // 1. Build commit DanceRequest (state is handled inside dance_call)
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 2. Call the dance
    let response = test_state.invoke_dance(request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. Validate response status and ResponseBody type
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "Commit request failed: {}",
        response.description
    );
    info!("Success! Commit succeeded");

    // 3. The response body is now a HolonReference::Transient to a CommitResponseType holon.
    let commit_response_body_reference = match response.body {
        ResponseBody::HolonReference(HolonReference::Transient(ref commit_ref)) => {
            commit_ref.clone()
        }
        other => panic!("Unexpected ResponseBody for commit: {:?}", other),
    };

    // TODO: Once TypeDescriptors are enabled, we should also check the HolonType of the ResponseBody

    // 4. Retrieve committed holons from the HolonsCommitted relationship.
    let committed_references = commit_response_body_reference
        .related_holons(context, CoreRelationshipTypeName::HolonsCommitted)
        .expect("Failed to read HolonsCommitted relationship");

    let committed_refs_guard = committed_references.read().unwrap();
    let commit_count: MapInteger = committed_refs_guard.get_count();
    debug!("Discovered {:?} committed holons", commit_count.0);

    // 5. Add committed holons to the test_state.created_holons map.
    let committed_refs_guard =
        committed_references.read().expect("Failed to read committed holons");
    for href in committed_refs_guard.get_members() {
        // Extract key from the SmartReference’s cached smart properties
        let key_string: MapString = href
            .key(context)
            .expect("Failed to read key from committed HolonReference")
            .expect("Committed holon missing key");

        // href *is already* a fully valid, saved HolonReference
        test_state.created_holons.insert(key_string.clone(), href.clone());

        info!("Committed holon: {}", key_string);
    }

    // 6. Optional: log a summary
    info!("Commit complete: {} holons committed", committed_refs_guard.get_count().0);

    // // 4. Extract saved Holons from response body and add them to `created_holons`
    // match response.body {
    //     ResponseBody::Holon(holon) => {
    //         let key =
    //             holon.key().expect("Holon should have a key").expect("Key should not be None");
    //         test_state.created_holons.insert(key, holon);
    //     }
    //     ResponseBody::Holons(holons) => {
    //         for holon in holons {
    //             let key =
    //                 holon.key().expect("Holon should have a key").expect("Key should not be None");
    //             test_state.created_holons.insert(key, holon);
    //         }
    //     }
    //     _ => panic!("Invalid ResponseBody: {:?}", response.body),
    // }
}
