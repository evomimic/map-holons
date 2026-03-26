use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use rstest::*;
use std::collections::BTreeMap;

use holons_test::harness::helpers::BOOK_KEY;

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _fixture_bindings,
    } = TestCaseInit::new(
        "DeleteHolon Testcase",
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.",
    );

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference = fixture_context.mutation().new_holon(Some(book_key.clone()))?;

    // Mint
    let mut book_properties = BTreeMap::new();
    book_properties.insert("Title".to_property_name(), BOOK_KEY.to_base_value());
    book_properties.insert("description".to_property_name(), "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_base_value());

    let book_step_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_transient_reference,
        book_properties,
        Some(book_key.clone()),
        None,
        Some("Creating book holon...".to_string()),
    )?;

    // Add a stage-holon step and capture its TestReference for later steps
    let staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons,
        book_step_token,
        None,
        Some("Staging book holon...".to_string()),
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, None, None)?;

    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before delete".to_string()),
    )?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(&mut fixture_holons, staged_token.clone(), None, None)?;

    // ADD STEP: DELETE HOLON - Invalid //
    // The client's delete_holon_internal wraps non-OK dance responses as HolonError::Misc,
    // so the original HolonNotFound variant is lost in the string wrapper.
    test_case.add_delete_holon_step(
        &mut fixture_holons,
        staged_token,
        Some(HolonErrorKind::Misc),
        Some("Attempting invalid delete...".to_string()),
    )?;

    // TODO: more robust handling of the implication of deletes on links needs to be implemented before this step will work
    // // ADD STEP:  ENSURE DATABASE COUNT
    // test_case.add_ensure_database_count_step( fixture_holons.count_saved())?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
