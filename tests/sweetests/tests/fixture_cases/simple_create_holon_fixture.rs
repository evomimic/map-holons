use std::collections::BTreeMap;

use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;
use holons_test::harness::helpers::{BOOK_KEY};

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
        let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, fixture_bindings: _fixture_bindings } =
        TestCaseInit::new(
            "Simple Create/Get Holon Testcase".to_string(),
            "Ensure the holons and relationships setup by book and author setup helper commit successfully".to_string(),
    );

    // Ensure DB count //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), Some("Ensuring DB is 'empty' (only contains initial LocalHolonSpace).".to_string()),)?;

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference = new_holon(&fixture_context, Some(book_key.clone()))?;

    let mut properties = BTreeMap::new();
    properties.insert("title".to_property_name(), BOOK_KEY.to_base_value());
    properties.insert("description".to_property_name(), "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_base_value());
    // Mint
    let book_source_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_transient_reference,
        properties,
        Some(book_key),
        ResponseStatusCode::OK,
        Some("Creating book holon... ".to_string()),
    )?;

    test_case.add_stage_holon_step(
        &mut fixture_holons,
        book_source_token.clone(),
        ResponseStatusCode::OK,
        Some("Staging book holon...".to_string()),
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&*fixture_context, &mut fixture_holons, ResponseStatusCode::OK, None)?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step( fixture_holons.count_saved(), None)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Finalize
   test_case.finalize(&fixture_context)?;


    Ok(test_case)
}
