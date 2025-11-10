use holons_test::{ResolvedTestReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function is intended to test the ability to remove holons from a specified relationship
/// originating at a source_token.
///
/// There are two levels of testing required.
/// 1. Removing related holons from an already staged holon.
/// 2. Removing related holons from a previously saved holon
///
/// The first is a local operation on the staged holon (it does not invoke any dances).
///
/// The second requires:
///     a. retrieving the source holon
///     b. either cloning it or staging a new version of it
///     c. removing the specified holons from the specified relationship
///     d. committing the changes
///     TODO:
///     e. confirming the new holon is no longer related to the holons to remove via the specified relationship.
///
pub async fn execute_remove_related_holons(
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    source_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Removing Related Holons ---");

     // 1) LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();
    let holons_to_remove: Vec<HolonReference> =
        state.lookup_holon_references(context, &holons).unwrap();

    // 2. BUILD - remove_related_holons DanceRequest
    let request = build_remove_related_holons_dance_request(
        source_reference,
        relationship_name,
        holons_to_remove,
    )
    .expect("Failed to build remove_related_holons request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response status
    assert_eq!(response.status_code, expected_response,
        "remove_related_holons request returned unexpected status: {}",
        response.description
    );
    info!("Success! Related Holons have been removed");
   
    // 4) RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    let resulting_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
    state.record_resolved(resolved_reference);
}
