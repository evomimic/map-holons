use holons_test::{harness::prelude::TestExecutionState, ResolvedTestReference, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holon references. Accepting holons as TestReferences allows the target holons to
/// be either StagedHolons or SavedHolons. In the latter case, the executor needs to resolve
/// the TestReference's key into a HolonReference
///

pub async fn execute_add_related_holons(
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    source_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Add Related Holons ---");

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();
    let holons_to_add: Vec<HolonReference> =
        state.lookup_holon_references(context, &holons).unwrap();

    // 2. BUILD — dance request to add related holons
    let request = build_add_related_holons_dance_request(
        source_reference.clone(),
        relationship_name,
        holons_to_add,
    )
    .expect("Failed to build add_related_holons request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "add_related_holons request returned unexpected status: {}",
        response.description
    );
    info!("Success! DanceResponse matched expected");

    // 5. ASSERT — on success, the body should be a HolonReference to the abandoned holon.
    //            Compare essential content
    let resulting_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
    resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Related Holons have been added");

    // 6. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(resolved_reference);
}
