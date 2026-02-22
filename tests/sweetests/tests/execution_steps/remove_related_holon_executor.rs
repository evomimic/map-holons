use holons_test::{ExecutionHandle, ExecutionReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function is intended to test the ability to remove holons from a specified relationship
/// originating at a step_token.
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
    state: &mut TestExecutionState,
    step_token: TestReference,
    relationship_name: RelationshipName,
    holons: Vec<TestReference>,
    expected_response: ResponseStatusCode,
    description: Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Removing Related Holons".to_string(),
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.resolve_source_reference(&context, &step_token).unwrap();
    let holons_to_remove: Vec<HolonReference> =
        state.resolve_source_references(&context, &holons).unwrap();

    // 2. BUILD - remove_related_holons DanceRequest
    let request = build_remove_related_holons_dance_request(
        source_reference,
        relationship_name,
        holons_to_remove.clone(),
    )
    .expect("Failed to build remove_related_holons request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "remove_related_holons request returned unexpected status: {}",
        response.description
    );
    info!("Success! Related Holons have been removed");

    // [future TODO]: when there is inverse relationship functionality, the dance response body should be changed
    // to return BOTH the source AND the target holon references. Then in step 6, those target references that were returned
    // will be used to create execution references to be recorded.
    if response.status_code == ResponseStatusCode::OK {
        // 5. ASSERT — execution-time content matches fixture expectation
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

        execution_reference.assert_essential_content_eq();
        info!("Success! Updated holon's essential content matched expected");

        // 6. RECORD — register execution result for downstream steps
        state.record(&step_token, execution_reference).unwrap();
        // [future TODO]: no longer use the same execution reference that was derived from source
        let mut index = 0;
        for token in holons {
            state
                .record(
                    &token,
                    ExecutionReference::from_token_execution(
                        &token,
                        ExecutionHandle::from(holons_to_remove[index].clone()),
                    ),
                )
                .unwrap();
            index += 1;
        }
    }
}
