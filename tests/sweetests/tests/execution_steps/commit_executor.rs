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
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Committing Staged Holons ---");

    // 1. BUILD - dance request to commit
    let request = build_commit_dance_request().expect("Failed to build commit DanceRequest");
    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "commit request returned unexpected status: {}",
        response.description
    );
    info!("Success! commit DanceResponse matched expected");

    // WHAT TODO: response body is a Holon need a Reference

    // // 5. Extract saved Holons from response body and add them to `created_holons`
    // match response.body {
    //     ResponseBody::Holon(holon) => {
    //         let key =
    //             holon.key().expect("Holon should have a key").expect("Key should not be None");
    //         state.execution_holons.record_resolved(
    //     }
    //     ResponseBody::Holons(holons) => {
    //         for holon in holons {
    //             let key =
    //                 holon.key().expect("Holon should have a key").expect("Key should not be None");
    //             state.execution_holons.record_resolved(resolved);.insert(key, holon);
    //         }
    //     }
    //     _ => panic!("Invalid ResponseBody: {:?}", response.body),
    // }

    // let resulting_reference = match response.body {
    //     ResponseBody::HolonReference(ref hr) => hr.clone(),
    //     other => {
    //         panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
    //     }
    // };
    // let resolved_reference =
    //     ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
    // resolved_reference.assert_essential_content_eq(context).unwrap();
    // info!("Success! Related Holons have been added");

    // // 4. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    // //             so later steps can look it up with the same token.
    // state.record_resolved(resolved_reference);
}
