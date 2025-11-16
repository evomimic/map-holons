use holons_test::{DancesTestCase, FixtureHolons};
use rstest::*;

use holons_prelude::prelude::*;

use crate::helpers::{BOOK_KEY, init_fixture_context};

use super::setup_book_author_steps_with_context;

/// Fixture for creating Simple NEWVERSION Testcase
#[fixture]
pub async fn simple_stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple StageNewVersion Testcase".to_string(),
        "Tests stage_new_version dance, \n\
        1. creates and commits a holon, clones it, changes some properties, \n \
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

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;


    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let _relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case, &mut fixture_holons)?;

    expected_count += staged_count(&*fixture_context).unwrap();

    // Get and set the various Holons data.
    let book_key = MapString(BOOK_KEY.to_string());

    // Mint a transient-intent token and index it by key so we can refer to it later.
    let transient_source = new_holon(&*fixture_context, MapString::from(book_key.clone()))?;
    let transient_source_token = fixture_holons
        .add_transient_with_key(&transient_source, book_key.clone())?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    //  ENSURE DATABASE COUNT  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step(ResponseStatusCode::OK)?;

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    test_case.add_stage_new_version_step(&mut fixture_holons, transient_source_token, Some(book_key), ResponseStatusCode::OK)?;
    // NOTE: Assume this test step executor actually stages TWO new versions from original
    expected_count += 2;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step(ResponseStatusCode::OK)?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
