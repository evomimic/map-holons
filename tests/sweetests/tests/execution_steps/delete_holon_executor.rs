use core_types::LocalId;
use holons_prelude::prelude::*;
use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holochain::sweettest::*;

/// This function builds and dances a `delete_holon` DanceRequest for the supplied Holon
/// and matches the expected response
///
pub async fn execute_delete_holon(
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Deleting an Existing (Saved) Holon");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        { state.resolve_source_reference(context, &source_token).unwrap() };

    let HolonId::Local(local_id) =
        source_reference.holon_id(context).expect("Failed to get HolonId")
    else {
        panic!("Expected LocalId");
    };

    // 2. BUILD - dance request to commit
    let request = build_delete_holon_dance_request(local_id.clone())
        .expect("Failed to build delete_holon request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "delete_holon request returned unexpected status: {}",
        response.description
    );
    info!("Success! Confirmed DanceResponse matched expected {:?}...", expected_status);

    // Confirm that the Holon has been successfully deleted
    let get_request = build_get_holon_by_id_dance_request(HolonId::Local(local_id))
        .expect("Failed to build get_holon_by_id request");

    let dance_initiator = context.get_dance_initiator().unwrap();
    let get_response = dance_initiator.initiate_dance(context, get_request).await;
    assert_eq!(
        get_response.status_code,
        ResponseStatusCode::NotFound,
        "Holon should be deleted but was found"
    );
    info!("Confirmed Holon deletion!");

    // 5. RECORD - Register an ExecutionHolon in a deleted state (does not resolve)

    if response.status_code == ResponseStatusCode::OK {
        let resulting_reference = ResultingReference::Deleted;
        let resolved_reference = ExecutionReference::from_reference_parts(
            source_token.expected_snapshot(),
            resulting_reference,
        );
        state.record(&source_token, resolved_reference).unwrap();
    } else {
            let resulting_reference = ResultingReference::LiveReference(source_reference);
            let resolved_reference = ExecutionReference::from_reference_parts(
                source_token.expected_snapshot(),
                resulting_reference,
            );
            state.record(&source_token, resolved_reference).unwrap();
    }
}
