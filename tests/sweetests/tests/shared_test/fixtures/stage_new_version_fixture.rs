use crate::shared_test::setup_book_author_steps_with_context;
use crate::shared_test::test_data_types::{DancesTestCase, BOOK_KEY};
use holons_core::core_shared_objects::HolonError;

use crate::shared_test::test_context::init_test_context;
use crate::shared_test::test_context::TestContextConfigOption::TestFixture;
use holons_core::dances::dance_response::ResponseStatusCode;
use holons_core::HolonReference;
use rstest::*;
use shared_types_holon::{BaseValue, MapInteger, MapString};

/// Fixture for creating Simple NEWVERSION Testcase
#[fixture]
pub fn simple_stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
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
    // will go away once Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_test_context(TestFixture);
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let _relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    expected_count += staging_service.borrow().staged_count();

    // Get and set the various Holons data.
    let book_key = MapString(BOOK_KEY.to_string());
    // let book_holon_ref = staging_service.get_staged_holon_by_base_key(fixture_context, &book_key)?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    let cloned_book_key =
        BaseValue::StringValue(MapString("A new version of: Emerging World".to_string()));

    test_case.add_stage_new_version_step(book_key, ResponseStatusCode::OK)?;
    // NOTE: Assume this test step executor actually stages TWO new versions from original
    expected_count += 2;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
