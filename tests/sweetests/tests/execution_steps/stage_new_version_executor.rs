use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holon_dance_builders::stage_new_version_dance::build_stage_new_version_dance_request;

// TODO: Version 2

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Staging a New Version of a Holon ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // VERSION 1 //

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(context, &source_token).unwrap();

    // Can only stage from Saved
    let smart_reference = match source_reference {
        HolonReference::Smart(smart_reference) => smart_reference,
        other => {
            panic!("{}", format!("expected lookup to return SmartReference, got {:?}", other));
        }
    };

    // 2. BUILD — stage_new_version DanceRequest
    let original_holon_id = smart_reference.holon_id(context).expect("Failed to get LocalId");
    let request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );

    // 5. ASSERT the staged holon's content matches
    let version_1_response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let version_1_resulting_reference =
        ResultingReference::from(version_1_response_holon_reference.clone());
    // TestReference::new()
    let version_1_resolved_reference = ExecutionReference::from_reference_parts(
        source_token.expected_snapshot(),
        version_1_resulting_reference.clone(),
    );
    version_1_resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Staged new version holon's essential content matched expected");

    // 6. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
    state.record(&source_token, version_1_resolved_reference);

    // 7. Verify the new version as the original holon as its predecessor
    let predecessor = version_1_response_holon_reference.predecessor(context).unwrap();

    assert_eq!(
        predecessor,
        Some(HolonReference::Smart(SmartReference::new(original_holon_id.clone(), None))),
        "Predecessor relationship did not match expected"
    );

    let original_holon_key = smart_reference.key(context).unwrap().unwrap();

    // 8. Verify new version's key matches original holon's key and that it is the ONLY staged
    // holon whose key matches.
    let by_base = get_staged_holon_by_base_key(context, &original_holon_key).unwrap();

    let version_1_holon_reference = version_1_resulting_reference
        .get_holon_reference()
        .expect("HolonReference must be Live, cannot be in a deleted state");
    assert_eq!(
        version_1_holon_reference,
        HolonReference::Staged(by_base),
        "get_staged_holon_by_base_key did not match expected"
    );

    // 9. Verify staged holon retrieval by versioned key
    let by_version = get_staged_holon_by_versioned_key(
        context,
        &version_1_holon_reference.versioned_key(context).unwrap(),
    )
    .unwrap();

    assert_eq!(
        version_1_holon_reference,
        HolonReference::Staged(by_version),
        "get_staged_holon_by_versioned_key did not match expected"
    );

    info!("Success! New version Holon matched expected content and relationships.");
}
