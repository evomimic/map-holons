use holons_test::{ResolvedTestReference, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

use holon_dance_builders::stage_new_version_dance::build_stage_new_version_dance_request;
use holons_core::core_shared_objects::ReadableHolonState; // Eliminate this dependency

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Staging a New Version of a Holon ---");

    // 1. Get context from test_state
    let ctx_arc = test_state.context(); // Arc lives until end of scope
    let context = ctx_arc.as_ref();

    // 1. Retrieve the original Holon
    let original_holon =
        test_state.get_created_holon_by_key(&original_holon_key).unwrap_or_else(|| {
            panic!("Holon with key {:?} not found in created_holons", original_holon_key)
        });

    let original_holon_id = original_holon.holon_id(context).expect("Failed to get LocalId");

    // 2. Build the DanceRequest
    // 1) LOOKUP — get the input handle for the source token
    // VERSION 1 //

    // 1. LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();

    // Can only stage Transient
    let transient_reference = match source_reference {
        HolonReference::Transient(tr) => tr,
        other => {
            panic!("{}", format!("expected lookup to return TransientReference, got {:?}", other));
        }
    };

    // 2. BUILD — stage_new_version DanceRequest
    let original_holon_id = transient_reference.holon_id(context).expect("Failed to get LocalId");
    let request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );

   // 5. ASSERT the staged holon's content matches
    let version_1_resulting_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let version_1_resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, version_1_resulting_reference);
    version_1_resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Staged new version holon's essential content matched expected");

    info!("New version Holon reference returned: {:?}", version_1);

    // 6. Verify the new version matches original version's essential content
    // Can't get essential_content from original_holon because it is only a reference
    // assert_eq!(
    //     original_holon.essential_content(context),
    //     version_1.essential_content(context),
    //     "New version Holon content did not match original"
    // );
    // 6) RECORD — tie the new staged handle to the **source token’s TemporaryId**
    // 6. RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(version_1_resolved_reference);

    
    // 7. Verify the new version as the original holon as its predecessor

    let predecessor = version_1.predecessor(context).unwrap();
    let predecessor = version_1_resulting_reference.predecessor(context).unwrap();

    assert_eq!(
        predecessor,
        Some(HolonReference::Smart(SmartReference::new(original_holon_id.clone(), None))),
        "Predecessor relationship did not match expected"
    );

    // 8. Verify new version's key matches original holon's key and that it is the ONLY staged
    // holon whose key matches.
    let by_base = get_staged_holon_by_base_key(context, &original_holon_key).unwrap();

    assert_eq!(
        version_1,
        HolonReference::Staged(by_base),
        "get_staged_holon_by_base_key did not match expected"
    );

    // 9. Verify staged holon retrieval by versioned key
    let by_version =
        get_staged_holon_by_versioned_key(context, &version_1.versioned_key(context).unwrap())
            .unwrap();

    assert_eq!(
        version_1,
        HolonReference::Staged(by_version),
        "get_staged_holon_by_versioned_key did not match expected"
    );

    info!("Success! New version Holon matched expected content and relationships.");


    // VERSION 2 //

    // Stage a second version from the same original holon in order to verify that:
    // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // b. get_staged_holons_by_base_key correctly returns BOTH stage holons
    let next_request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");
    debug!("2nd Dance Request: {:#?}", next_request);

    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let next_response = dance_initiator.initiate_dance(context, request).await;
    info!("2nd Dance Response: {:#?}", next_response.clone());

    assert_eq!(
        next_response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        next_response.description
    );

    // Extract the second new version holon from the response
    let version_2_resulting_reference = match next_response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    let version_2_resolved_reference =
        ResolvedTestReference::from_reference_parts(source_token, version_2_resulting_reference);
    // Ensure essential content is preserved
    // assert_eq!(
    //     original_holon.essential_content(context),
    //     version_2.essential_content(context),
    //     "New version Holon content did not match original"
    // );
    version_2_resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Staged new version holon's essential content matched expected");

    // Record resolved
    state.record_resolved(version_2_resolved_reference);

    // Confirm that get_staged_holon_by_versioned_key returns the new version
    let versioned_lookup =
        get_staged_holon_by_versioned_key(context, &version_2.versioned_key(context).unwrap())
            .unwrap();

    assert_eq!(
        version_2,
        HolonReference::Staged(versioned_lookup),
        "get_staged_holon_by_versioned_key did not match expected"
    );

    info!("Success! Second new version Holon matched expected content and relationships.");

    // Confirm that get_staged_holon_by_base_key returns a duplicate error.
    let book_holon_staged_reference_result =
        get_staged_holon_by_base_key(context, &original_holon_key)
            .expect_err("Expected duplicate error");
    assert_eq!(
        HolonError::DuplicateError(
            "Holons".to_string(),
            "key: Emerging World: The Evolution of Consciousness and the Future of Humanity"
                .to_string()
        ),
        book_holon_staged_reference_result
    );

    // Confirm that get_staged_holons_by_base_key returns two staged references for the two versions.
    let book_holon_staged_references =
        get_staged_holons_by_base_key(context, &original_holon_key).unwrap();
    let holon_references: Vec<HolonReference> =
        book_holon_staged_references.iter().map(|h| HolonReference::Staged(h.clone())).collect();
    assert_eq!(
        book_holon_staged_references.len(),
        2,
        "get_staged_holons_by_base_key should return two staged references"
    );
    assert_eq!(
        vec![version_1, version_2],
        holon_references,
        "Fetched staged references did not match expected"
    );
}
