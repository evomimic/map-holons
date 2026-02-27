use holons_test::{
    ExecutionHandle, ExecutionReference, ResolveBy, TestExecutionState, TestReference,
};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function builds and dances a `with_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) with_properties dance returns with a Success
/// 2) the returned HolonReference refers to a Holon's essential_content that matches the expected
///

pub async fn execute_with_properties(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
    description: String,
) {
    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    // 2. BUILD — with_properties DanceRequest

    let request = build_with_properties_dance_request(source_reference.clone(), properties.clone())
        .expect("Failed to build with_properties request");

    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "with_properties request returned unexpected status: {}",
        response.description
    );
    info!("Success! with_properties DanceResponse matched expected");

    if response.status_code != ResponseStatusCode::OK {
        return;
    }

    // Success only after this point

    // 5. ASSERT — essential content matches expected
    let response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => panic!("expected ResponseBody::HolonReference, got {:?}", other),
    };

    let execution_handle = ExecutionHandle::from(response_holon_reference);

    let execution_reference =
        ExecutionReference::from_token_execution(&step_token, execution_handle);

    execution_reference.assert_essential_content_eq();
    info!("Success! Updated holon's essential content matched expected");

    // 6. RECORD — make this execution result available downstream
    state.record(&step_token, execution_reference).unwrap();
}
