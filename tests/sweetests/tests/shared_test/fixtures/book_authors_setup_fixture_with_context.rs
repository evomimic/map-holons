#![allow(dead_code)]
use crate::shared_test::{
    test_context::init_fixture_context,
    test_data_types::{
        DancesTestCase, TestReference, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY,
        PERSON_2_KEY, PUBLISHER_KEY,
    },
};
use holons_prelude::prelude::*;

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use core_types::{PropertyName, RelationshipName};
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, TransientReference, WritableHolon,
    },
};

use tracing::{debug, info};
// Import the test-only extension
use std::string::ToString; // Import the test-only extension
use type_names::property_names::*;
use type_names::relationship_names::ToRelationshipName;
use type_names::CorePropertyTypeName::Description;

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
) -> Result<RelationshipName, HolonError> {
    // Set relationship
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();

    //  STAGE:  Book Holon  //
    let book_holon_key = MapString(BOOK_KEY.to_string());

    let book_transient_reference = new_holon(&*fixture_context, book_holon_key.clone())?;
    book_transient_reference.with_property_value(&*fixture_context, "title", BOOK_KEY)?.with_property_value(
            &*fixture_context,
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            )))?;

    info!(
        "================= In setup_book_author_steps_with_context. Here's the book: \n{:?}",
        book_transient_reference.essential_content(fixture_context)?
    );

    test_case.add_stage_holon_step(book_transient_reference.clone())?;

    let book_staged_reference = stage_new_holon(&*fixture_context, book_transient_reference)?;

    // //  STAGE:  Person 1 //
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let person_1_transient_reference = new_holon(&*fixture_context, person_1_key.clone())?;
    person_1_transient_reference
        .with_property_value(
            &*fixture_context,
            MapString("first name".to_string()),
            "Roger".to_string(),
        )?
        .with_property_value(&*fixture_context, "last name".to_string(), "Briggs".to_string())?;
    test_case.add_stage_holon_step(person_1_transient_reference.clone())?;

    let person_1_staged_reference =
        stage_new_holon(&*fixture_context, person_1_transient_reference.clone())?;

    //  STAGE:  Person 2 //
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let person_2_transient_reference = new_holon(&*fixture_context, person_2_key.clone())?;
    person_2_transient_reference.with_property_value(
        &*fixture_context,
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("George".to_string())),
    )?;
    person_2_transient_reference.with_property_value(
        &*fixture_context,
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Smith".to_string())),
    )?;
    test_case.add_stage_holon_step(person_2_transient_reference.clone())?;

    let person_2_staged_reference =
        stage_new_holon(&*fixture_context, person_2_transient_reference)?;

    //  STAGE:  Publisher //
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_transient_reference = new_holon(&*fixture_context, publisher_key.clone())?;
    publisher_transient_reference.with_property_value(
        &*fixture_context,
        Description,
        "We publish Holons for testing purposes",
    )?;
    test_case.add_stage_holon_step(publisher_transient_reference.clone())?;

    let _publisher_staged_reference =
        stage_new_holon_api(&*fixture_context, publisher_transient_reference)?;

    //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    let mut fixture_target_references: Vec<HolonReference> = Vec::new();
    fixture_target_references.push(HolonReference::from_staged(person_1_staged_reference.clone()));
    fixture_target_references.push(HolonReference::from_staged(person_2_staged_reference.clone()));

    book_staged_reference.add_related_holons(
        &*fixture_context,
        BOOK_TO_PERSON_RELATIONSHIP,
        fixture_target_references.clone(),
    )?;

    let mut target_references: Vec<TestReference> = Vec::new();
    target_references.push(TestReference::StagedHolon(person_1_staged_reference));
    target_references.push(TestReference::StagedHolon(person_2_staged_reference));

    let expected_holon = HolonReference::Staged(book_staged_reference.clone());

    // Create the expected_holon
    test_case.add_related_holons_step(
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
