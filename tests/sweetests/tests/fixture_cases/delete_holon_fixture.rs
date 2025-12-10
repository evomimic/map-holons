use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureHolons};
use rstest::*;

use crate::helpers::{init_fixture_context, BOOK_KEY};

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());
    let mut book_transient_reference = new_holon(&*fixture_context, Some(book_key.clone()))?;
    book_transient_reference.with_property_value(
        &*fixture_context,
        "title".to_string(),
        BOOK_KEY,
    )?.with_property_value(
            &*fixture_context,
            "description",
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?",
            )?;

    // Mint a transient-intent token and index it by key.
    let transient_source_token = fixture_holons.add_transient_with_key(
        &book_transient_reference,
        book_key.clone(),
        &book_transient_reference.essential_content(&*fixture_context)?,
    )?;

    // Stage
    test_case.add_stage_holon_step(
        &mut fixture_holons,
        transient_source_token.clone(),
        Some(book_key.clone()),
        ResponseStatusCode::OK,
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    let saved_token = fixture_holons.get_latest_by_key(&book_key)?;

    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(
        &mut fixture_holons,
        saved_token.clone(),
        ResponseStatusCode::OK,
    )?;

    // // ADD STEP: DELETE HOLON - Invalid //
    // test_case.add_delete_holon_step(
    //     &mut fixture_holons,
    //     saved_token,
    //     ResponseStatusCode::NotFound,
    // )?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
