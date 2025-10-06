// #![allow(dead_code)]

use rstest::*;

use crate::shared_test::{
    setup_book_author_steps_with_context,
    test_context::init_fixture_context,
    test_data_types::{DancesTestCase, TestReference, BOOK_KEY, PERSON_1_KEY},
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, PropertyName};
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, StagedReference, TransientReference,
        WritableHolon,
    },
};

// use holons_core::prelude::*;
use type_names::relationship_names::ToRelationshipName;

/// Fixture for creating Simple AbandonStagedChanges Testcase
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    //== INIT ==//

    let mut test_case = DancesTestCase::new(
        "Simple AbandonStagedChanges Testcase".to_string(),
        "Tests abandon_staged_changes dance, confirms behavior of commit and verifies abandoned holon is not accessible".to_string(),
    );

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either key
    let fixture_context = init_fixture_context();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    expected_count += staged_count(&*fixture_context);

    let person_1_staged_reference =
        get_staged_holon_by_base_key(&*fixture_context, &MapString(PERSON_1_KEY.to_string()))?;

    let book_staged_reference =
        get_staged_holon_by_base_key(&*fixture_context, &MapString(BOOK_KEY.to_string()))?;

    //====//``

    //  ABANDON:  H2  //
    // This step verifies the abandon dance succeeds and that subsequent operations on the
    // abandoned Holon return NotAccessible Errors
    test_case.add_abandon_staged_changes_step(
        HolonReference::Staged(person_1_staged_reference.clone()),
        ResponseStatusCode::OK,
    )?;
    expected_count -= 1;

    //  RELATIONSHIP:  Author H2 -> H3  //
    // Attempt add_related_holon dance -- expect Conflict/NotAccessible response
    let holons_to_add: Vec<TestReference> = Vec::new();
    test_case.add_related_holons_step(
        HolonReference::Staged(person_1_staged_reference), // source holons
        "FRIENDS".to_relationship_name(),
        holons_to_add.to_vec(),
        ResponseStatusCode::Conflict,
        HolonReference::Transient(book_staged_reference.clone_holon(&*fixture_context)?),
    )?;

    //  COMMIT  //  all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    //  STAGE:  Abandoned Holon1 (H4)  //
    let mut abandoned_holon_1_transient_reference =
        create_empty_transient_holon(&*fixture_context, MapString("Abandon1".to_string()))?;
    abandoned_holon_1_transient_reference.with_property_value(
        &*fixture_context,
        "example abandon1",
        "test1",
    )?;
    test_case.add_stage_holon_step(abandoned_holon_1_transient_reference.clone())?;

    let abandoned_holon_1_staged_reference =
        stage_new_holon_api(&*fixture_context, abandoned_holon_1_transient_reference)?;
    expected_count += 1;

    //  STAGE:  Abandoned Holon2 (H5)  //
    let mut abandoned_holon_2_transient_reference =
        create_empty_transient_holon(&*fixture_context, MapString("Abandon2".to_string()))?;
    abandoned_holon_2_transient_reference.with_property_value(
        &*fixture_context,
        "example abandon2",
        "test2",
    )?;
    test_case.add_stage_holon_step(abandoned_holon_2_transient_reference.clone())?;

    let abandoned_holon_2_staged_reference =
        stage_new_holon_api(&*fixture_context, abandoned_holon_2_transient_reference)?;
    expected_count += 1;

    // ABANDON:  H4
    test_case.add_abandon_staged_changes_step(
        HolonReference::Staged(abandoned_holon_1_staged_reference),
        ResponseStatusCode::OK,
    )?;
    expected_count -= 1;

    // ABANDON:  H5
    test_case.add_abandon_staged_changes_step(
        HolonReference::Staged(abandoned_holon_2_staged_reference),
        ResponseStatusCode::OK,
    )?;
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
        MapString(BOOK_KEY.to_string()),
        query_expression,
        ResponseStatusCode::OK,
    )?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
