use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;
use std::collections::BTreeMap;

use super::setup_book_author_steps_with_context;

/// Fixture for creating Simple AbandonStagedChanges Testcase
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    //== INIT ==//

    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        mut fixture_bindings,
    } = TestCaseInit::new(
            "Simple AbandonStagedChanges Testcase".to_string(),
            "Tests abandon_staged_changes dance, confirms behavior of commit and verifies abandoned holon is not accessible".to_string(),
        );

    // // Ensure DB count //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved())?;

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    setup_book_author_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    let _relationship_name =
        fixture_bindings.relationship_by_name(&MapString("BOOK_TO_PERSON".to_string())).unwrap();

    let person_1_staged_token =
        fixture_bindings.get_token(&MapString("Person1".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Person1' label").clone();

    //  ABANDON:  H2  //
    // This step verifies the abandon dance succeeds and that subsequent operations on the
    // abandoned Holon return NotAccessible Errors
    let _abandoned_person_1 = test_case.add_abandon_staged_changes_step(
        &mut fixture_holons,
        person_1_staged_token,
        ResponseStatusCode::OK,
    )?;

    // //  RELATIONSHIP:  Author H2 -> H3  //
    // // Attempt add_related_holon dance -- expect Conflict/NotAccessible response
    // let holons_to_add: Vec<TestReference> = Vec::new();
    // test_case.add_add_related_holons_step(
    //     &*fixture_context,
    //     abandoned_person_1,
    //     "FRIENDS".to_relationship_name(),
    //     holons_to_add.to_vec(),
    //     ResponseStatusCode::Conflict,
    // )?;

    //  COMMIT  //  all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(fixture_holons.count_saved())?;

    //  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    //  STAGE:  Abandoned Holon1 (H4)  //
    let abandoned_holon_1_key = MapString("Abandon1".to_string());
    let abandoned_holon_1_transient_reference =
        new_holon(&fixture_context, Some(abandoned_holon_1_key.clone()))?;

    // Mint
    let mut abandon1_properties = BTreeMap::new();
    abandon1_properties.insert("example abandon1".to_property_name(), "test1".to_base_value());

    let abandoned_holon_1_transient_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        abandoned_holon_1_transient_reference,
        abandon1_properties,
        Some(abandoned_holon_1_key.clone()),
        ResponseStatusCode::OK,
    )?;
    // Add a stage-holon step and capture its TestReference for later steps
    let abandoned_holon_1_staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons,
        abandoned_holon_1_transient_token,
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Abandoned Holon2 (H5)  //
    let abandoned_holon_2_key = MapString("Abandon2".to_string());
    let abandoned_holon_2_transient_reference =
        new_holon(&fixture_context, Some(abandoned_holon_2_key.clone()))?;
    // Mint
    let mut abandon2_properties = BTreeMap::new();
    abandon2_properties.insert("example abandon2".to_property_name(), "test2".to_base_value());

    let abandoned_holon_2_transient_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        abandoned_holon_2_transient_reference,
        abandon2_properties,
        Some(abandoned_holon_2_key.clone()),
        ResponseStatusCode::OK,
    )?;
    // Add a stage-holon step and capture its TestReference for later steps
    let abandoned_holon_2_staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons,
        abandoned_holon_2_transient_token,
        ResponseStatusCode::OK,
    )?;

    // ABANDON:  H4
    test_case.add_abandon_staged_changes_step(
        &mut fixture_holons,
        abandoned_holon_1_staged_token,
        ResponseStatusCode::OK,
    )?;

    // ABANDON:  H5
    test_case.add_abandon_staged_changes_step(
        &mut fixture_holons,
        abandoned_holon_2_staged_token,
        ResponseStatusCode::OK,
    )?;

    // COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(fixture_holons.count_saved())?;

    // MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    // // ADD STEP: QUERY RELATIONSHIPS //
    // let query_expression = QueryExpression::new(relationship_name.clone());
    // let book_staged_token =
    //     fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Book' label").clone();
    // test_case.add_query_relationships_step(
    //     book_staged_token,
    //     query_expression,
    //     ResponseStatusCode::OK,
    // )?;

    // Finalize
    test_case.finalize(&fixture_context)?;


    Ok(test_case)
}
