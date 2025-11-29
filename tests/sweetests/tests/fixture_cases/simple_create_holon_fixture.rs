use crate::{
    fixture_cases::setup_book_author_steps_with_context,
    helpers::{init_fixture_context, BOOK_KEY},
};
use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureHolons};
use rstest::*;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure the holons and relationships setup by book and author setup helper commit successfully".to_string(),
    );

    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();


    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());
    let mut book_transient_reference = new_holon(&*fixture_context, Some(book_key.clone()))?;
    book_transient_reference.with_property_value(&*fixture_context, "title".to_string(), BOOK_KEY,)?.with_property_value(&*fixture_context, "description", "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?")?;

    // Mint a transient-intent token and index it by key.
    let transient_source_token = fixture_holons.add_transient_with_key(
        &book_transient_reference,
        book_key.clone(),
        &book_transient_reference.essential_content(&*fixture_context)?,
    )?;

    test_case.add_stage_holon_step(
        &mut fixture_holons,
        transient_source_token.clone(),
        Some(book_key),
        ResponseStatusCode::OK,
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
