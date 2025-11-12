use holons_test::{test_case::DancesTestCase, test_reference::TestReference, FixtureHolons};
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
pub fn setup_book_author_steps_with_context(
    fixture_context: &dyn HolonsContextBehavior,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
) -> Result<RelationshipName, HolonError> {
    // Set relationship
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();

    //  STAGE:  Book Holon  //
    let book_key = MapString(BOOK_KEY.to_string());

    let mut book_transient_reference = new_holon(&*fixture_context, Some(book_holon_key.clone()))?;
    let mut book_transient_reference = new_holon(&*fixture_context, book_key.clone())?;
    book_transient_reference.with_property_value(&*fixture_context, "title", BOOK_KEY)?.with_property_value(
            &*fixture_context,
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            )))?;

    let book_transient_token =
        fixture_holons.add_transient_with_key(&book_transient_reference, book_holon_key)?;
    let book_staged_reference = test_case.add_stage_holon_step(
        fixture_holons,
        person_1_transient_reference,
        book_key,
        ResponseStatusCode::OK,
    )?;

    // //  STAGE:  Person 1 //
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let mut person_1_transient_reference =
        new_holon(&*fixture_context, Some(person_1_key.clone()))?;
    person_1_transient_reference
        .with_property_value(&*fixture_context, "first name", "Roger")?
        .with_property_value(&*fixture_context, "last name", "Briggs")?;
    let person_1_staged_reference = test_case.add_stage_holon_step(
        fixture_holons,
        person_1_transient_reference,
        person_1_key,
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Person 2 //
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let mut person_2_transient_reference =
        new_holon(&*fixture_context, Some(person_2_key.clone()))?;
    person_2_transient_reference
        .with_property_value(&*fixture_context, "first name", "George")?
        .with_property_value(&*fixture_context, "last name", "Smith")?;
    let person_2_staged_reference = test_case.add_stage_holon_step(
        fixture_holons,
        person_2_transient_reference,
        person_2_key,
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Publisher //
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let mut publisher_transient_reference =
        new_holon(&*fixture_context, Some(publisher_key.clone()))?;
    publisher_transient_reference
        .with_property_value(&*fixture_context, "name", PUBLISHER_KEY)?
        .with_property_value(
            &*fixture_context,
            Description,
            "We publish Holons for testing purposes",
        )?;
    let _publisher_staged_reference = test_case.add_stage_holon_step(
        fixture_holons,
        publisher_transient_reference,
        publisher_key,
        ResponseStatusCode::OK,
    )?;

    //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    let mut fixture_target_references: Vec<HolonReference> = Vec::new();
    fixture_target_references.push(HolonReference::from_staged(person_1_staged_reference.clone()));
    fixture_target_references.push(HolonReference::from_staged(person_2_staged_reference.clone()));

    book_staged_reference.transient().add_related_holons(
        &*fixture_context,
        BOOK_TO_PERSON_RELATIONSHIP,
        fixture_target_references.clone(),
    )?;

    let mut target_references: Vec<TestReference> = Vec::new();
    target_references.push(TestReference::StagedHolon(person_1_staged_reference));
    target_references.push(TestReference::StagedHolon(person_2_staged_reference));

    let expected_holon = HolonReference::Staged(book_staged_reference.clone());

    // Create the expected_holon
    test_case.add_add_related_holons_step(
        HolonReference::Staged(book_staged_reference),
        relationship_name.clone(),
        target_references,
        ResponseStatusCode::OK,
        expected_holon,
    )?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(relationship_name)
}
