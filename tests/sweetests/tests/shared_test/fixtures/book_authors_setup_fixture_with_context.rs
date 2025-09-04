#![allow(dead_code)]

use crate::shared_test::{
    test_context::init_fixture_context,
    test_data_types::{
        DancesTestCase, TestReference, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY,
        PERSON_2_KEY, PUBLISHER_KEY,
    },
};
use crate::warn;
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, ReadableHolonReferenceLayer,
        TransientReference, WriteableHolon, WriteableHolonReferenceLayer,
    },
};
use integrity_core_types::{PropertyName, RelationshipName};
use std::string::ToString; // Import the test-only extension
use type_names::relationship_names::ToRelationshipName;

// pub struct TestHolon {
//     pub key: MapString,
//     pub expected_holon: Option<Holon>,
// }

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
    context: &dyn HolonsContextBehavior,
    test_case: &mut DancesTestCase,
) -> Result<RelationshipName, HolonError> {
    // Set relationship
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();

    // Init test context
    let fixture_context = init_fixture_context();

    // Get transient manager behavior
    let transient_manager_behavior_service =
        fixture_context.get_space_manager().get_transient_behavior_service();
    let transient_manager_behavior = transient_manager_behavior_service.borrow();

    // Get transient manager access
    let transient_manager_access =
        TransientReference::get_transient_manager_access(&*fixture_context);
    let transient_manager = transient_manager_access.borrow();

    //  STAGE:  Book Holon  //
    let book_holon_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference =
        transient_manager_behavior.create_empty(book_holon_key.clone())?;
    let book_id = book_transient_reference.get_temporary_id();

    let rc_book_holon = transient_manager.get_holon_by_id(&book_id)?;

    book_transient_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("title".to_string())),
            BaseValue::StringValue(book_holon_key.clone()),
        )?
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            )))?;
    test_case.test_session_state.holons.insert(book_id.clone(), rc_book_holon.borrow().clone());
    test_case.test_session_state.keyed_index.insert(book_holon_key, book_id);
    test_case.add_stage_holon_step(book_transient_reference.clone())?;

    let book_staged_reference = stage_new_holon_api(context, book_transient_reference)?;

    // //  STAGE:  Person 1 //
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let person_1_transient_reference =
        transient_manager_behavior.create_empty(person_1_key.clone())?;
    let person_1_id = person_1_transient_reference.get_temporary_id();

    let rc_person_1 = transient_manager.get_holon_by_id(&person_1_id)?;

    person_1_transient_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("Roger".to_string())),
        )?
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Briggs".to_string())),
        )?;
    test_case.test_session_state.holons.insert(person_1_id.clone(), rc_person_1.borrow().clone());
    test_case.test_session_state.keyed_index.insert(person_1_key.clone(), person_1_id);
    test_case.add_stage_holon_step(person_1_transient_reference.clone())?;

    let person_1_staged_reference =
        stage_new_holon_api(context, person_1_transient_reference.clone())?;

    //  STAGE:  Person 2 //
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let person_2_transient_reference =
        transient_manager_behavior.create_empty(person_2_key.clone())?;
    let person_2_id = person_1_transient_reference.get_temporary_id();

    let rc_person_2 = transient_manager.get_holon_by_id(&person_2_id)?;

    person_2_transient_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("George".to_string())),
        )?
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Smith".to_string())),
        )?;
    test_case.test_session_state.holons.insert(person_2_id.clone(), rc_person_2.borrow().clone());
    test_case.test_session_state.keyed_index.insert(person_2_key.clone(), person_2_id);
    test_case.add_stage_holon_step(person_2_transient_reference.clone())?;

    let person_2_staged_reference = stage_new_holon_api(context, person_2_transient_reference)?;

    //  STAGE:  Publisher //
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_transient_reference =
        transient_manager_behavior.create_empty(publisher_key.clone())?;
    let publisher_id = publisher_transient_reference.get_temporary_id();

    let rc_publisher = transient_manager.get_holon_by_id(&publisher_id)?;

    publisher_transient_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("name".to_string())),
            BaseValue::StringValue(publisher_key.clone()),
        )?
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString("We publish Holons for testing purposes".to_string())),
        )?;
    test_case.test_session_state.holons.insert(publisher_id.clone(), rc_publisher.borrow().clone());
    test_case.test_session_state.keyed_index.insert(publisher_key, publisher_id);
    test_case.add_stage_holon_step(publisher_transient_reference.clone())?;

    stage_new_holon_api(context, publisher_transient_reference)?;

    //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    let mut fixture_target_references: Vec<HolonReference> = Vec::new();
    fixture_target_references.push(HolonReference::from_staged(person_1_staged_reference.clone()));
    fixture_target_references.push(HolonReference::from_staged(person_2_staged_reference.clone()));

    book_staged_reference.add_related_holons(
        context,
        BOOK_TO_PERSON_RELATIONSHIP,
        fixture_target_references.clone(),
    )?;

    let mut target_references: Vec<TestReference> = Vec::new();
    target_references.push(TestReference::StagedHolon(person_1_staged_reference));
    target_references.push(TestReference::StagedHolon(person_2_staged_reference));

    // Create the expected_holon
    test_case.add_related_holons_step(
        book_staged_reference.clone(),
        relationship_name.clone(),
        target_references,
        ResponseStatusCode::OK,
        Holon::Transient(rc_book_holon.borrow().clone().into_transient()?),
    )?;

    Ok(relationship_name)
}
