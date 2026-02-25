use holon_dance_builders::remove_properties_dance::build_remove_properties_dance_request;
use holons_prelude::prelude::*;
use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use tracing::{debug, info};

/// This function builds and dances a `remove_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) remove_properties dance returns with a Success
/// 2) the returned HolonReference refers to a Holon's essential_content that matches the expected
///

pub async fn execute_remove_properties(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
    description: Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Removing Properties from Holon".to_string(),
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. BUILD — remove_properties DanceRequest
    let request = build_remove_properties_dance_request(source_reference, properties.clone())
        .expect("Failed to build remove_properties request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "remove_properties request returned unexpected status: {}",
        response.description
    );
    info!("Success! remove_properties DanceResponse matched expected");

    if response.status_code == ResponseStatusCode::OK {
        // 5. ASSERT — updated holon's essential content matches expected
        let response_holon_reference = match response.body {
            ResponseBody::HolonReference(ref hr) => hr.clone(),
            other => {
                panic!("expected ResponseBody::HolonReference, got {:?}", other);
            }
        };

        // Build execution handle from runtime result
        let execution_handle = ExecutionHandle::from(response_holon_reference);

        // Canonical construction: token + execution outcome
        let execution_reference =
            ExecutionReference::from_token_execution(&step_token, execution_handle);

        // Validate expected vs execution-time content
        execution_reference.assert_essential_content_eq();
        info!("Success! Updated holon's essential content matched expected");

        // 6. RECORD — make this execution result available for downstream steps
        state.record(&step_token, execution_reference).unwrap();
    }
}
