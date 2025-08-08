#![allow(dead_code)]

use std::string::ToString; // Import the test-only extension

use crate::shared_test::test_data_types::{
    DancesTestCase, TestReference, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY,
    PERSON_2_KEY, PUBLISHER_KEY,
};

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, ReadableHolonReferenceLayer,
        WriteableHolon, WriteableHolonReferenceLayer,
    },
};
use integrity_core_types::{PropertyName, RelationshipName};
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
    let relationship_name =
        MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string()).to_relationship_name();

    //  STAGE:  Book Holon  //
    let mut book_holon = TransientHolon::new();
    let book_holon_key = MapString(BOOK_KEY.to_string()); // ✅ Convert to MapString

    book_holon
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(book_holon_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("title".to_string())),
            Some(BaseValue::StringValue(book_holon_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            Some(BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            ))),
        )?;
    test_case.add_stage_holon_step(book_holon.clone())?;
    let book_ref = stage_new_holon_api(context, book_holon)?;

    //  STAGE:  Person 1 //
    let mut person_1_holon = TransientHolon::new();
    let person_1_key = MapString(PERSON_1_KEY.to_string()); // ✅ Convert to MapString

    person_1_holon
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            Some(BaseValue::StringValue(MapString("Roger".to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            Some(BaseValue::StringValue(MapString("Briggs".to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(person_1_key.clone())),
        )?;
    test_case.add_stage_holon_step(person_1_holon.clone())?;
    let person_1_reference = stage_new_holon_api(context, person_1_holon.clone())?;

    //  STAGE:  Person 2 //
    let mut person_2_holon = TransientHolon::new();
    let person_2_key = MapString(PERSON_2_KEY.to_string()); // ✅ Convert to MapString

    person_2_holon
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            Some(BaseValue::StringValue(MapString("George".to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            Some(BaseValue::StringValue(MapString("Smith".to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(person_2_key.clone())),
        )?;
    test_case.add_stage_holon_step(person_2_holon.clone())?;
    let person_2_reference = stage_new_holon_api(context, person_2_holon.clone())?;

    //  STAGE:  Publisher //
    let mut publisher_holon = TransientHolon::new();
    let publisher_key = MapString(PUBLISHER_KEY.to_string()); // ✅ Convert to MapString

    publisher_holon
        .with_property_value(
            PropertyName(MapString("name".to_string())),
            Some(BaseValue::StringValue(publisher_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(publisher_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            Some(BaseValue::StringValue(MapString(
                "We publish Holons for testing purposes".to_string(),
            ))),
        )?;
    test_case.add_stage_holon_step(publisher_holon.clone())?;
    stage_new_holon_api(context, publisher_holon.clone())?;

    //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    let mut fixture_target_references: Vec<HolonReference> = Vec::new();
    fixture_target_references.push(HolonReference::from_staged(person_1_reference.clone()));
    fixture_target_references.push(HolonReference::from_staged(person_2_reference.clone()));

    book_ref.add_related_holons(
        context,
        BOOK_TO_PERSON_RELATIONSHIP,
        fixture_target_references.clone(),
    )?;

    let mut target_references: Vec<TestReference> = Vec::new();
    target_references.push(TestReference::StagedHolon(person_1_reference));
    target_references.push(TestReference::StagedHolon(person_2_reference));

    // Create the expected_holon
    test_case.add_related_holons_step(
        book_ref.clone(),
        relationship_name.clone(),
        target_references,
        ResponseStatusCode::OK,
        Holon::Transient(book_ref.clone_holon(context)?),
    )?;

    Ok(relationship_name)
}
