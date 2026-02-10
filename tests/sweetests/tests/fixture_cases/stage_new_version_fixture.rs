use std::iter::Map;

use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;

use holons_prelude::prelude::*;
// use tracing::debug;

use super::setup_book_author_steps_with_context;
use holons_test::harness::helpers::BOOK_KEY;

// TODO: add/remove relationships

/// Fixture for creating Simple NEWVERSION Testcase
#[fixture]
pub fn stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings } =
        TestCaseInit::new(
            "Simple StageNewVersion Testcase".to_string(),
            "Tests stage_new_version dance".to_string(),
        );
    let mut version_count = MapInteger(1);

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    setup_book_author_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    let book_staged_token = fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Book' label").clone();

    //  ENSURE DATABASE COUNT -- Initial //
    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some("Ensuring DB is 'empty' (only contains initial LocalHolonSpace).".to_string()),
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ResponseStatusCode::OK,
        Some("Commit --- after setup_book_authors".to_string()),
    )?;

    //  ENSURE DATABASE COUNT -- After Commit //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Get book source
    let book_key = MapString(BOOK_KEY.to_string());

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    let staged_clone = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        ResponseStatusCode::OK,
        version_count.clone(),
        None,
        Some("Stage New Version -- first clone from book.".to_string()),
    )?;

    // Add properties
    let mut expected_clone_properties = PropertyMap::new();
    expected_clone_properties.insert("Key".to_property_name(), book_key.clone().to_base_value());
    expected_clone_properties.insert(
        "Description".to_property_name(),
        "This is a different description".to_base_value(),
    );
    expected_clone_properties.insert("title".to_property_name(), "Changed".to_base_value());

    test_case.add_with_properties_step(
        &mut fixture_holons,
        staged_clone,
        expected_clone_properties.clone(),
        ResponseStatusCode::OK,
        Some("With Properties -- first version cloned from book.".to_string()),
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ResponseStatusCode::OK,
        Some("With Properties -- first version cloned from book.".to_string()),
    )?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // VERSION 2 //
    // Stage a second version from the same original holon in order to verify that:
    // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // b. get_staged_holons_by_base_key correctly returns BOTH staged holons

    let _version_2_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        ResponseStatusCode::OK,
        version_count.clone(),
        None,
        Some("Stage New Version --- second version".to_string()),
    )?;
    version_count.0 += 1;

    let _version_3_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token,
        ResponseStatusCode::OK,
        version_count.clone(),
        Some(ResponseStatusCode::ServerError),
        Some("Stage New Version --- third version, expecting ServerError for duplicate return of get_staged_holon_by_base_key".to_string())
    )?;
    version_count.0 += 1;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
