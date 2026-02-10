use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holon_dance_builders::stage_new_version_dance::build_stage_new_version_dance_request;

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_response: ResponseStatusCode,
    description: Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Staging New Version of a Holon".to_string(),
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(&context, &source_token).unwrap();

    // 2. BUILD — stage_new_version DanceRequest
    let original_holon_id = source_reference.holon_id()
        .expect("Failed to get LocalId");
    let request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request)
        .await;
    // let dance_initiator = context.get_dance_initiator().unwrap();
    // let response = dance_initiator.initiate_dance(Arc::clone(&context), request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );
    info!("Success! stage_new_version DanceResponse matched expected");

    if response.status_code != ResponseStatusCode::OK {
        return;
    }

    // ---- success path only beyond this point ----

    // 5. ASSERT — response body must be a HolonReference
    let response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => panic!("expected ResponseBody::HolonReference, got {:?}", other),
    };

    let execution_handle = ExecutionHandle::from(response_holon_reference.clone());

    let execution_reference =
        ExecutionReference::from_token_execution(&source_token, execution_handle.clone());

    execution_reference.assert_essential_content_eq();
    info!("Success! Staged new version holon's essential content matched expected");

    // 6. RECORD — make execution result available downstream
    state.record(&source_token, execution_reference).unwrap();

    // 7. Verify the new version has the original holon as its predecessor.
    let predecessor = response_holon_reference.predecessor().unwrap().unwrap();

    assert_eq!(
        predecessor.holon_id().unwrap(),
        original_holon_id,
        "Predecessor relationship did not match expected"
    );

    // 8. Verify base-key staging behavior
    let original_holon_key = source_reference.key().unwrap().unwrap();
    let by_base = get_staged_holon_by_base_key(&context, &original_holon_key).unwrap();

    let staged_reference = execution_handle
        .get_holon_reference()
        .expect("HolonReference must be live");

    assert_eq!(
        staged_reference,
        HolonReference::Staged(by_base),
        "get_staged_holon_by_base_key did not match expected"
    );

    // 9. Verify versioned-key lookup
    let by_version = get_staged_holon_by_versioned_key(
        &context,
        &staged_reference.versioned_key()
.unwrap(),
    )
        .unwrap();

    assert_eq!(
        staged_reference,
        HolonReference::Staged(by_version),
        "get_staged_holon_by_versioned_key did not match expected"
    );

    info!("Success! New version Holon matched expected content and relationships.");
}
