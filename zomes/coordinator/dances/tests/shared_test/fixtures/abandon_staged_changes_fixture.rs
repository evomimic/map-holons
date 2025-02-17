#![allow(dead_code)]

// use crate::get_holon_by_key_from_test_state;
use crate::tracing::{error, info, warn};
use core::panic;
use std::cell::RefCell;
//use holochain::core::author_key_is_valid;

use crate::shared_test::setup_book_author_steps_with_context;
use crate::shared_test::test_data_types::DancesTestCase;
use dances::dance_response::ResponseStatusCode;
use holons::reference_layer::HolonReference::Staged;
use holons::reference_layer::{HolonReference, StagedReference};
use holons_client::init_client_context;
use holons_core::core_shared_objects::{Holon, HolonCollection, HolonError, RelationshipName};
use holons_core::{HolonReadable, HolonsContextBehavior};
use holons_guest::query_layer::QueryExpression;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

/// Fixture for creating Simple AbandonStagedChanges Testcase
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple AbandonStagedChanges Testcase".to_string(),
        "Tests abandon_staged_changes dance, confirms behavior of commit and verifies abandoned holon is not accessible".to_string(),
    );

    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_client_context().as_ref();
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.

    let relationship_name = setup_book_author_steps_with_context(fixture_context, &mut test_case)?;
    expected_count += staging_service.borrow().staged_count();

    let person_1_key = MapString("Roger Briggs".to_string());
    let person_1_ref = staging_service.borrow().get_staged_holon_by_key(person_1_key)?;

    let book_key = MapString("Emerging World".to_string());
    let book_ref = staging_service.borrow().get_staged_holon_by_key(book_key.clone())?;

    //  ABANDON:  H2  //
    // This step verifies the abandon dance succeeds and that subsequent operations on the
    // abandoned Holon return NotAccessible Errors
    test_case.add_abandon_staged_changes_step(person_1_ref.clone(), ResponseStatusCode::OK)?;
    expected_count -= 1;

    //  RELATIONSHIP:  Author H2 -> H3  //
    // Attempt add_related_holon dance -- expect Conflict/NotAccessible response
    test_case.add_related_holons_step(
        person_1_ref, // source holons
        RelationshipName(MapString("FRIENDS".to_string())),
        holons_to_add.to_vec(),
        ResponseStatusCode::Conflict,
        book_ref.clone_holon(fixture_context)?,
    )?;

    //  COMMIT  //  all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    //  STAGE:  Abandoned Holon1 (H4)  //
    let mut abandoned_holon_1 = Holon::new();
    abandoned_holon_1.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("Abandon1".to_string())),
    )?;
    abandoned_holon_1.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test1".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_1.clone())?;
    expected_count += 1;

    //  STAGE:  Abandoned Holon2 (H5)  //
    let mut abandoned_holon_2 = Holon::new();
    abandoned_holon_2.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("Abandon2".to_string())),
    )?;
    abandoned_holon_2.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test2".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_2.clone())?;
    expected_count += 1;

    // ABANDON:  H4
    test_case
        .add_abandon_staged_changes_step(StagedReference::from_index(0), ResponseStatusCode::OK)?;
    expected_count -= 1;

    // ABANDON:  H5
    test_case
        .add_abandon_staged_changes_step(StagedReference::from_index(1), ResponseStatusCode::OK)?;
    expected_count -= 1;

    // COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    // ADD STEP: QUERY RELATIONSHIPS //
    let query_expression = QueryExpression::new(relationship_name.clone());
    test_case.add_query_relationships_step(
        book_key.clone(),
        query_expression,
        ResponseStatusCode::OK,
    )?;

    Ok(test_case.clone())
}
