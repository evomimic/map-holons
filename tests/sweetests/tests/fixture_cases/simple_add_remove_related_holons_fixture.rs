use crate::fixture_cases::setup_book_author_steps_with_context;
use holons_core::{core_shared_objects::WritableRelationship, CollectionState};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY, PERSON_2_KEY, PUBLISHER_KEY,
};
use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;
use std::collections::BTreeMap;
use tracing::info;
// TODO: enhance test capabilities, ie trying to remove related holons using invalid source and relationship name

/// For both Transient and Staged references:
/// adds a new relationship, removes an existing relationship, then adds another relationship again.
///
///
#[fixture]
pub fn simple_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings } =
        TestCaseInit::new(
            "Simple Add / Remove Related Holon Testcase".to_string(),
            "Tests the adding and removing of related Holons".to_string(),
        );
    // let _ = holochain_trace::test_run();

    // Ensure DB count //
    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some("Ensuring DB is 'empty' (only contains initial LocalHolonSpace).".to_string()),
    )?;

    // Use helper function to stage Book, 2 Person, 1 Publisher Holon and AUTHORED_BY relationship
    // from the book to the two persons
    setup_book_author_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    info!("fixture: book and author setup complete.");

    // let book_transient_reference = fixture_holons.get_latest_by_key(&book_key).unwrap().transient();

    // === TRANSIENT === //  Company -> HOST -> Website  //
    let host_relationship = "HOST".to_relationship_name();
    let company_key = MapString("COMPANY_KEY".to_string());
    let website_key = MapString("WEBSITE_KEY".to_string());
    // Create transient references
    let company_transient_reference = new_holon(&fixture_context, Some(company_key.clone()))?;
    let mut company_properties = BTreeMap::new();
    company_properties
        .insert("name".to_property_name(), "The Really Useful Information Company".to_base_value());
    let website_transient_reference = new_holon(&fixture_context, Some(website_key.clone()))?;
    let mut website_properties = BTreeMap::new();
    website_properties.insert("url".to_property_name(), "itsyourworld.com".to_base_value());
    // Mint
    let company_step_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        company_transient_reference,
        company_properties,
        Some(company_key),
        ResponseStatusCode::OK,
        Some("Creating company holon... ".to_string()),
    )?;
    let website_step_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        website_transient_reference,
        website_properties,
        Some(website_key),
        ResponseStatusCode::OK,
        Some("Creating website holon... ".to_string()),
    )?;

    // -- ADD STEP -- //
    // Set expected
    company_step_token.expected_reference().add_related_holons(
        &host_relationship,
        vec![HolonReference::from(website_transient_reference.clone())],
    )?;
    // Executor step
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        company_step_token.clone(),
        host_relationship.clone(),
        vec![website_step_token.clone()],
        ResponseStatusCode::OK,
        Some("Adding Relationship:  Company -> HOST -> Website ".to_string()),
    )?;

    // -- REMOVE STEP -- //
    // Set expected
    company_step_token.expected_reference().remove_related_holons(
        &host_relationship,
        vec![HolonReference::from(website_transient_reference.clone())],
    )?;
    // Executor step
    test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        company_step_token,
        host_relationship.clone(),
        vec![website_transient_token.clone()],
        ResponseStatusCode::OK,
        Some("Removing Relationship:  Company -> HOST -> Website ".to_string()),
    )?;

    // // -- Again ADD STEP -- //
    // let again_relationship = "AGAIN".to_relationship_name();
    // let example_key = MapString("EXAMPLE_KEY".to_string());
    // let example_transient_reference = new_holon(&fixture_context, Some(example_key.clone()))?;
    // let example_transient_token = fixture_holons.add_transient(
    //     &example_transient_reference,
    //     example_key.clone(),
    //     &example_transient_reference.essential_content(&fixture_context)?,
    // )?;
    // // Set expected
    // company_expected_content
    //     .relationships
    //     .add_related_holons(
    //         &fixture_context,
    //         CollectionState::Transient,
    //         again_relationship.clone(),
    //         vec![HolonReference::from(example_transient_reference.clone())],
    //     )
    //     .unwrap();
    // // Mint another snapshot
    // let another_company_token = fixture_holons.add_transient(
    //     &company_transient_reference,
    //     company_key.clone(),
    //     &company_expected_content,
    // )?;
    // // Add step
    // test_case.add_add_related_holons_step(
    //     company_transient_token.clone(),
    //     again_relationship.clone(),
    //     vec![example_transient_token.clone()],
    //     ResponseStatusCode::OK,
    // )?;

    //  COMMIT  //
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK, None)?;

    // ENSURE DB COUNT //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    // //  QUERY RELATIONSHIPS  //
    // let query_expression = QueryExpression::new(host_relationship.clone());
    // test_case.add_query_relationships_step(
    //     company_step_token,
    //     query_expression,
    //     ResponseStatusCode::OK,
    //     None,
    // )?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
