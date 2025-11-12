use holon_dance_builders::remove_properties_dance::build_remove_properties_dance_request;
use holons_prelude::prelude::*;
use holons_test::{ResolvedTestReference, TestExecutionState, TestReference};
use tracing::{debug, info};

/// This function builds and dances a `remove_properties` DanceRequest for the supplied Holon
/// To pass this test, all the following must be true:
/// 1) remove_properties dance returns with a Success
/// 2) the returned HolonReference refers to a Holon's essential_content that matches the expected
///

pub async fn execute_remove_properties(
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    source_token: TestReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Removing Properties from Holon ---");

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();

    // 2. BUILD — remove_properties DanceRequest
    let request = build_remove_properties_dance_request(source_reference, properties.clone())
        .expect("Failed to build remove_properties request");
    debug!("Dance Request: {:#?}", request);
    
    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "remove_properties request returned unexpected status: {}",
        response.description
    );

    // 5. ASSERT - updated holon's essential content matches expected
    let resulting_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
    resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Updated holon's essential content matched expected");

    // 6. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(resolved_reference);

}
