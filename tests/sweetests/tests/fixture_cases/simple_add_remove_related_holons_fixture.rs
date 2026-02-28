use crate::fixture_cases::setup_book_author_steps_with_context;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::PUBLISHED_BY;
use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;
use std::collections::BTreeMap;
use tracing::info;
// TODO: enhance test capabilities, ie trying to remove related holons using invalid source and relationship name

/// For both Transient and Staged references:
/// adds a new relationship, removes an existing relationship, then adds another relationship again.
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

    let book_to_person_relationship =
        fixture_bindings.relationship_by_name(&MapString("BOOK_TO_PERSON".to_string())).unwrap();

    let book_staged_token =
        fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Book' label").clone();

    let person_1_staged_token =
        fixture_bindings.get_token(&MapString("Person1".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Person1' label").clone();

    let publisher_staged_token =
        fixture_bindings.get_token(&MapString("Publisher".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Publisher' label").clone();

    // === TRANSIENT === //
    //
    // Company -> HOST -> Website  //
    //
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
    let company_token_after_add = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        company_step_token.clone(),
        host_relationship.clone(),
        vec![website_step_token.clone()],
        ResponseStatusCode::OK,
        Some("Adding Relationship (Transient):  Company -> HOST -> Website ".to_string()),
    )?;

    // -- REMOVE STEP -- //
    let company_token_after_remove = test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        company_token_after_add,
        host_relationship.clone(),
        vec![website_step_token.clone()],
        ResponseStatusCode::OK,
        Some("Removing Relationship (Transient):  Company -> HOST -> Website ".to_string()),
    )?;

    // -- Again ADD STEP -- //
    let again_relationship = "AGAIN".to_relationship_name();
    let example_key = MapString("EXAMPLE_KEY".to_string());
    // Create example
    let example_transient_reference = new_holon(&fixture_context, Some(example_key.clone()))?;
    let mut example_properties = BTreeMap::new();
    example_properties.insert("example".to_property_name(), "Example Holon".to_base_value());
    // Mint
    let example_step_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        example_transient_reference,
        example_properties,
        Some(example_key),
        ResponseStatusCode::OK,
        Some("Creating example holon... ".to_string()),
    )?;
    // Executor step
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        company_token_after_remove,
        again_relationship.clone(),
        vec![example_step_token.clone()],
        ResponseStatusCode::OK,
        Some("Adding Relationship (Transient):  Company -> AGAIN -> Example ".to_string()),
    )?;
    //
    // == //

    // === STAGED === //
    //
    // -- REMOVE STEP -- //
    // Book -> AUTHORED_BY -> Person1  //
    let book_token_after_remove = test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        book_to_person_relationship.clone(),
        vec![person_1_staged_token.clone()],
        ResponseStatusCode::OK,
        Some("Removing Relationship (Staged):  Book -> AUTHORED_BY -> Person1 ".to_string()),
    )?;

    // -- ADD STEP -- //
    // Book -> PUBLISHED_BY -> Publisher
    let book_token_after_add = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token_after_remove,
        PUBLISHED_BY.to_relationship_name(),
        vec![publisher_staged_token.clone()],
        ResponseStatusCode::OK,
        Some("Adding Relationship (Staged):  Book -> PUBLISHED_BY -> Publisher ".to_string()),
    )?;
    //
    // == //

    //  COMMIT  //
    test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK, None)?;

    // ENSURE DB COUNT //
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    //  QUERY RELATIONSHIPS  //
    let query_expression = QueryExpression::new(book_to_person_relationship.clone());
    test_case.add_query_relationships_step(
        &mut fixture_holons,
        book_token_after_add,
        query_expression,
        ResponseStatusCode::OK,
        None,
    )?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
