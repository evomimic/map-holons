use holons_test::{
    dance_test_language::DancesTestCase, test_reference::TestReference, FixtureHolons,
};
use tracing::{debug, info};

use holons_prelude::prelude::*;
use type_names::CorePropertyTypeName::Description;

use crate::helpers::{
    init_fixture_context, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY, PERSON_2_KEY,
    PUBLISHER_KEY,
};

/// This function updates the supplied test_case with a set of steps that establish some basic
/// data the different test cases can then extend for different purposes.
/// Specifically, this function stages 4 Holons (but does NOT commit) and creates 1 Relationship, with the following test data:
/// *Book Holon* with BOOK_KEY title
/// *Person Holon with PERSON_1_KEY*
/// *Person Holon with PERSON_2_KEY*
/// *Publisher Holon with PUBLISHER_KEY*
/// *BOOK_TO_PERSON_RELATIONSHIP from Book Holon to person 1 and person 2
/// The Nursery within the supplied context is used as the test data setup area
///
/// FixtureHolons contains the minted token TestReferences that are used to track a lineage of state to mirror in parallel the ExecutionHolons.
/// This parallel reflects 'expected' (Fixture) vs 'actual' (Mock DHT).
pub fn setup_book_author_steps_with_context(
    fixture_context: &dyn HolonsContextBehavior,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
) -> Result<RelationshipName, HolonError> {
    // Set relationship
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();

    //  STAGE:  Book Holon  //
    //
    // Create fresh holon
    let book_key = MapString(BOOK_KEY.to_string());
    let mut book_transient_reference = new_holon(&*fixture_context, Some(book_key.clone()))?;
    book_transient_reference.with_property_value(&*fixture_context, "title", BOOK_KEY)?.with_property_value(
            &*fixture_context,
            "Description",
            "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?",
            )?;

    // Mint TestReferences (tokens)
    let book_transient_token = fixture_holons.add_transient_with_key(
        &book_transient_reference.clone(),
        book_key.clone(),
        book_transient_reference.clone(),
    );
    let book_staged_token = test_case.add_stage_holon_step(
        &*fixture_context,
        fixture_holons,
        book_transient_token,
        Some(book_key),
        ResponseStatusCode::OK,
    )?;

    // //  STAGE:  Person 1 //
    //
    // Create
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let mut person_1_transient_reference =
        new_holon(&*fixture_context, Some(person_1_key.clone()))?;
    person_1_transient_reference
        .with_property_value(&*fixture_context, "first name", "Roger")?
        .with_property_value(&*fixture_context, "last name", "Briggs")?;
    // Mint
    let person_1_transient_token = fixture_holons.add_transient_with_key(
        &person_1_transient_reference.clone(),
        person_1_key.clone(),
        person_1_transient_reference.clone(),
    );
    let person_1_staged_token = test_case.add_stage_holon_step(
        &*fixture_context,
        fixture_holons,
        person_1_transient_token,
        Some(person_1_key),
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Person 2 //
    //
    // Create
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let mut person_2_transient_reference =
        new_holon(&*fixture_context, Some(person_2_key.clone()))?;
    person_2_transient_reference
        .with_property_value(&*fixture_context, "first name", "George")?
        .with_property_value(&*fixture_context, "last name", "Smith")?;
    // Mint
    let person_2_transient_token = fixture_holons.add_transient_with_key(
        &person_2_transient_reference.clone(),
        person_2_key.clone(),
        person_2_transient_reference.clone(),
    );
    let person_2_staged_token = test_case.add_stage_holon_step(
        &*fixture_context,
        fixture_holons,
        person_2_transient_token,
        Some(person_2_key),
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Publisher //
    //
    // Create
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let mut publisher_transient_reference =
        new_holon(&*fixture_context, Some(publisher_key.clone()))?;
    publisher_transient_reference
        .with_property_value(&*fixture_context, "name", PUBLISHER_KEY)?
        .with_property_value(
            &*fixture_context,
            "Description",
            "We publish Holons for testing purposes",
        )?;
    // Mint
    let publisher_transient_token = fixture_holons.add_transient_with_key(
        &publisher_transient_reference.clone(),
        publisher_key.clone(),
        publisher_transient_reference.clone(),
    );
    let _publisher_staged_token = test_case.add_stage_holon_step(
        &*fixture_context,
        fixture_holons,
        publisher_transient_token,
        Some(publisher_key),
        ResponseStatusCode::OK,
    )?;

    // //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    // let mut fixture_target_references: Vec<HolonReference> = Vec::new();
    // fixture_target_references.push(HolonReference::Transient(person_1_transient_reference.clone()));
    // fixture_target_references.push(HolonReference::Transient(person_2_transient_reference.clone()));

    // book_transient_reference.add_related_holons(
    //     &*fixture_context,
    //     BOOK_TO_PERSON_RELATIONSHIP,
    //     fixture_target_references.clone(),
    // )?;

    // let mut target_references: Vec<TestReference> = Vec::new();
    // target_references.push(person_1_staged_token);
    // target_references.push(person_2_staged_token);

    // test_case.add_add_related_holons_step(
    //     book_staged_token,
    //     relationship_name.clone(),
    //     target_references,
    //     ResponseStatusCode::OK,
    // )?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(relationship_name)
}
