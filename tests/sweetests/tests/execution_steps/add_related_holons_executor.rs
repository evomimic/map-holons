use holons_test::{
    harness::prelude::TestExecutionState, ExecutionReference, ResultingReference, TestReference,
};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holon references. Accepting holons as TestReferences allows the target holons to
/// be either StagedHolons or SavedHolons. In the latter case, the executor needs to resolve
/// the TestReference's key into a HolonReference
///

pub async fn execute_add_related_holons(
    state: &mut TestExecutionState,
    source_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Add Related Holons ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(context, &source_token).unwrap();
    let holons_to_add: Vec<HolonReference> =
        state.resolve_source_references(context, &holons).unwrap();

    // 2. BUILD — dance request to add related holons
    let request = build_add_related_holons_dance_request(
        source_reference.clone(),
        relationship_name,
        holons_to_add,
    )
    .expect("Failed to build add_related_holons request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "add_related_holons request returned unexpected status: {}",
        response.description
    );
    info!("Success! add_related_holons DanceResponse status matched expected");

    // 5. ASSERT - if Ok response expected, confirm essential content matches expected
    if response.status_code == ResponseStatusCode::OK {
        let response_holon_reference = match response.body {
            ResponseBody::HolonReference(ref hr) => hr.clone(),
            other => {
                panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
            }
        };
        let resulting_reference = ResultingReference::from(response_holon_reference);
        let resolved_reference = ExecutionReference::from_reference_parts(
            source_token.expected_snapshot(),
            resulting_reference,
        );

        resolved_reference.assert_essential_content_eq(context).unwrap();
        info!("Success! Updated holon's essential content matched expected");

        // 5. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
        state.record(source_token.expected_id(), resolved_reference);
    }
}
