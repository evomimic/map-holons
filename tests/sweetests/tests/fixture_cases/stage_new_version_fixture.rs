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
    let book_transient_reference = book_saved_token.transient().clone();

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    let _staged_clone = test_case.add_stage_new_version_step(
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
    book_clone_expected_content.relationships = EssentialRelationshipMap::default();

    // Mint
    let book_staged_token = fixture_holons.add_staged_with_key(
        &book_transient_reference,
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

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
