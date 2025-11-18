use holons_test::{ResolvedTestReference, TestExecutionState, TestReference};
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
    source_token: TestReference,
    properties: PropertyMap,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Updating Holon with Properties ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the source token
    let mut source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();

    // 2. BUILD — with_properties DanceRequest

    let request = build_with_properties_dance_request(source_reference.clone(), properties.clone())
        .expect("Failed to build with_properties request");

    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "with_properties request returned unexpected status: {}",
        response.description
    );

    // 5. ASSERT -
    // Create the expected holon by applying the property updates
    for (property_name, base_value) in properties.clone() {
        source_reference
            .with_property_value(context, property_name.clone(), base_value.clone())
            .expect("Failed to add property value to expected holon");
    }

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

    // 5. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(resolved_reference);
}
