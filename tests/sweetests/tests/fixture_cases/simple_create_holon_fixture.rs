use crate::{fixture_cases::setup_book_author_steps_with_context, helpers::init_fixture_context};
use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureHolons};
use rstest::*;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub async fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure the holons and relationships setup by book and author setup helper commit successfully".to_string(),
    );

    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();
    
    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count = MapInteger(1);

    // Ensure DB count //
    test_case.add_ensure_database_count_step(expected_count)?;

    // TODO:
    // const TRANSIENT_SOURCE_KEY: &str = "book:transient-source";
    // let transient_source =
    //     new_holon(fixture_context.as_ref(), MapString::from(TRANSIENT_SOURCE_KEY))?;

    // // Mint a transient-intent token and index it by key so we can refer to it later.
    // let transient_source_token = fixture_holons
    //     .add_transient_with_key(&transient_source, MapString::from(TRANSIENT_SOURCE_KEY))?;

    // // Use helper function to set up a book holon, 2 persons, a publisher, and a relationship from
    // // the book to both persons. Note that this uses the fixture's Nursery as a place to hold the test data.

    let _author_relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case, &mut fixture_holons)?;

    // // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // // This allows them to be assigned StagedReferences and also retrieved by either index or key

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    expected_count.0 += staged_count(&*fixture_context).unwrap();

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(expected_count)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step(ResponseStatusCode::OK)?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
