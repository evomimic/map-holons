use holons_core::core_shared_objects::holon::EssentialRelationshipMap;
use holons_test::{DancesTestCase, FixtureHolons};
use rstest::*;

use holons_prelude::prelude::*;
use tracing::warn;

use crate::helpers::{init_fixture_context, BOOK_KEY};

use super::setup_book_author_steps_with_context;

// TODO: add/remove relationships

/// Fixture for creating Simple NEWVERSION Testcase
#[fixture]
pub fn stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple StageNewVersion Testcase".to_string(),
        "Tests stage_new_version dance, \n\
        1. creates and commits a holon, clones it for staged, changes some properties, \n \
        2. adds and removes some relationships, \n\
        3. commits it and then compares essential content of existing holon and cloned holon"
            .to_string(),
    );

    // Initialize a client context the fixture can use
    // NOTE: This context will NOT be shared by test executors. The fixture's client context
    // includes a TransientHolonManager that is used as a scratch pad while in the fixture.
    // This allows them to be assigned TransientReferences and also retrieved by either index or key
    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let _relationship_name = setup_book_author_steps_with_context(
        &*fixture_context,
        &mut test_case,
        &mut fixture_holons,
    )?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    //  ENSURE DATABASE COUNT  //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Get book source
    let book_key = MapString(BOOK_KEY.to_string());
    let book_saved_token = fixture_holons.get_latest_by_key(&book_key)?;
    let mut book_transient_reference = book_saved_token.transient().clone();

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    let staged_clone = test_case.add_stage_new_version_step(
        &*fixture_context,
        &mut fixture_holons,
        book_saved_token,
        Some(book_key.clone()),
        ResponseStatusCode::OK,
    )?;

    // Set expected
    let mut expected_clone_properties = PropertyMap::new();
    expected_clone_properties.insert("Key".to_property_name(), book_key.clone().to_base_value());
    expected_clone_properties.insert(
        "Description".to_property_name(),
        "This is a different description".to_base_value(),
    );
    expected_clone_properties.insert("title".to_property_name(), "Changed".to_base_value());
    let mut book_clone_expected_content =
        book_transient_reference.essential_content(&*fixture_context)?;
    book_clone_expected_content.property_map = expected_clone_properties.clone();
    // book_clone_expected_content.relationships = EssentialRelationshipMap::default();

    // Mint
    let book_staged_token = fixture_holons.add_staged_with_key(
        &staged_clone.transient(),
        book_key.clone(),
        &book_clone_expected_content,
    )?;

    // Add properties
    test_case.add_with_properties_step(
        book_staged_token,
        expected_clone_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // TODO: Future issue: convert this code that was originally done in an execution step into this fixture as a stage_new 2nd pass
    // // VERSION 2 //

    // // Stage a second version from the same original holon in order to verify that:
    // // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // // b. get_staged_holons_by_base_key correctly returns BOTH stage holons
    // let next_request = build_stage_new_version_dance_request(original_holon_id.clone())
    //     .expect("Failed to build stage_new_version request");
    // debug!("2nd Dance Request: {:#?}", next_request);

    // let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    // let next_response = dance_initiator.initiate_dance(context, next_request).await;
    // info!("2nd Dance Response: {:#?}", next_response.clone());

    // assert_eq!(
    //     next_response.status_code, expected_response,
    //     "stage_new_version request returned unexpected status: {}",
    //     next_response.description
    // );

    // // Extract the second new version holon from the response
    // let version_2_response_holon_reference = match next_response.body {
    //     ResponseBody::HolonReference(ref hr) => hr.clone(),
    //     other => {
    //         panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
    //     }
    // };
    // let version_2_resulting_reference =
    //     ResultingReference::from(version_2_response_holon_reference.clone());
    // let version_2_resolved_reference = ResolvedTestReference::from_reference_parts(
    //     source_token,
    //     version_2_resulting_reference.clone(),
    // );

    // version_2_resolved_reference.assert_essential_content_eq(context).unwrap();
    // info!("Success! Staged new version holon's essential content matched expected");

    // // Record resolved
    // state.record_resolved(version_2_resolved_reference);

    // // Confirm that get_staged_holon_by_versioned_key returns the new version
    // let versioned_lookup = get_staged_holon_by_versioned_key(
    //     context,
    //     &version_2_response_holon_reference.versioned_key(context).unwrap(),
    // )
    // .unwrap();

    // let version_2_holon_reference = version_2_resulting_reference
    //     .get_holon_reference()
    //     .expect("HolonReference must be Live, cannot be in a deleted state");
    // assert_eq!(
    //     version_2_holon_reference,
    //     HolonReference::Staged(versioned_lookup),
    //     "get_staged_holon_by_versioned_key did not match expected"
    // );

    // info!("Success! Second new version Holon matched expected content and relationships.");

    // // Confirm that get_staged_holon_by_base_key returns a duplicate error.
    // let book_holon_staged_reference_result =
    //     get_staged_holon_by_base_key(context, &original_holon_key)
    //         .expect_err("Expected duplicate error");
    // assert_eq!(
    //     HolonError::DuplicateError(
    //         "Holons".to_string(),
    //         "key: Emerging World: The Evolution of Consciousness and the Future of Humanity"
    //             .to_string()
    //     ),
    //     book_holon_staged_reference_result
    // );

    // // Confirm that get_staged_holons_by_base_key returns two staged references for the two versions.
    // let book_holon_staged_references =
    //     get_staged_holons_by_base_key(context, &original_holon_key).unwrap();
    // let holon_references: Vec<HolonReference> =
    //     book_holon_staged_references.iter().map(|h| HolonReference::Staged(h.clone())).collect();
    // assert_eq!(
    //     book_holon_staged_references.len(),
    //     2,
    //     "get_staged_holons_by_base_key should return two staged references"
    // );
    // assert_eq!(
    //     vec![version_1_holon_reference, version_2_holon_reference],
    //     holon_references,
    //     "Fetched staged references did not match expected"
    // );

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
