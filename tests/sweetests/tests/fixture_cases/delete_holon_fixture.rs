use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use rstest::*;
use std::collections::BTreeMap;

use holons_test::harness::helpers::{BOOK_DESCRIPTOR_KEY, BOOK_KEY};

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

    // Deletion is descriptor-governed. Load the test-only Book descriptor,
    // then create a conforming Book instance whose type allows deletion.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for described Book setup".to_string()),
    )?;

    let book_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let book_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        book_type_stub,
        MapString(BOOK_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference = fixture_context.mutation().new_holon(Some(book_key.clone()))?;

    // Mint
    let mut book_properties = BTreeMap::new();
    book_properties.insert("Title".to_property_name(), BOOK_KEY.to_base_value());

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

    let staged_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        staged_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![book_type_token],
        None,
        Some("Describe Book by Book.HolonType".to_string()),
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before delete".to_string()),
    )?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(&mut fixture_holons, staged_token.clone(), None, None)?;

    // ADD STEP: DELETE HOLON - Invalid //
    test_case.add_delete_holon_step(
        &mut fixture_holons,
        staged_token,
        Some(HolonErrorKind::HolonNotFound),
        Some("Attempting invalid delete...".to_string()),
    )?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
