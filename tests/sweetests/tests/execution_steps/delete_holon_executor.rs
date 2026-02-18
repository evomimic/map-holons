use holons_prelude::prelude::*;
use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

/// This function builds and dances a `delete_holon` DanceRequest for the supplied Holon
/// and matches the expected response
///
pub async fn execute_delete_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    expected_status: ResponseStatusCode,
    description: Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Deleting an Existing (Saved) Holon".to_string()
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        { state.resolve_source_reference(&context, &step_token)
.unwrap() };

    let HolonId::Local(local_id) = source_reference.holon_id().expect("Failed to get HolonId")
    else {
        panic!("Expected LocalId");
    };

    // 2. BUILD - dance request to commit
    let request = build_delete_holon_dance_request(local_id.clone())
        .expect("Failed to build delete_holon request");
    debug!("Dance Request: {:#?}", request);

    let dance_initiator = context.get_dance_initiator().unwrap();

    // 3. CALL - the dance
    // Clone context and initiator for async call so they can still be used later
    let cloned_context = context.clone();
    let cloned_initiator = dance_initiator.clone();

    let response = cloned_initiator.initiate_dance(&cloned_context, request).await;
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
    let get_response = dance_initiator.initiate_dance(&context, get_request).await;
    assert_eq!(
        get_response.status_code,
        ResponseStatusCode::NotFound,
        "Holon should be deleted but was found"
    );
    info!("Confirmed Holon deletion!");

    // 5. RECORD — Register an ExecutionHolon reflecting the execution outcome

    let execution_handle = if response.status_code == ResponseStatusCode::OK {
        ExecutionHandle::Deleted
    } else {
        ExecutionHandle::from(source_reference)
    };

    let execution_reference =
        ExecutionReference::from_token_execution(&step_token, execution_handle);

    state.record(&step_token, execution_reference).unwrap();

}
