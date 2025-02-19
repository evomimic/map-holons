#![allow(dead_code)]

use core::panic;
use holochain::core::author_key_is_valid;

use crate::shared_test::test_data_types::DancesTestCase;
use crate::test_extensions::TestContextExtensions;

use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;
use std::string::ToString; // Import the test-only extension

use holons_core::core_shared_objects::holon_pool::HolonPool;
use holons_core::core_shared_objects::{
    Holon, HolonCollection, HolonError, RelationshipName, TransientCollection,
};
use holons_core::dances::dance_response::ResponseStatusCode;
use holons_core::staged_reference::StagedIndex;
use holons_core::{
    stage_new_holon_api, HolonReadable, HolonReference, HolonWritable, HolonsContextBehavior,
};
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};

pub struct TestHolon {
    pub staged_index: StagedIndex,
    pub key: MapString,
    pub expected_holon: Option<Holon>,
}

// These constants allow consistency between the helper function and its callers
pub const BOOK_KEY: MapString = MapString(
    "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
);
pub const PERSON_1_KEY: MapString = MapString("Roger Briggs".to_string());
pub const PERSON_2_KEY: MapString = MapString("George Smith".to_string());
pub const PUBLISHER_KEY: MapString = MapString("Publishing Company".to_string());
pub const BOOK_TO_PERSON_RELATIONSHIP: RelationshipName =
    RelationshipName(MapString("AUTHORED_BY".to_string()));

/// This function updates the supplied test_case with a set of steps that establish some basic
/// data the different test cases can then extend for different purposes.
/// Specifically, this function stages (but does NOT commit) the following test data:
/// *Book Holon* with BOOK_KEY title
/// *Person Holon with PERSON_1_KEY*
/// *Person Holon with PERSON_2_KEY*
/// *BOOK_TO_PERSON_RELATIONSHIP from Book Holon to person 1 and person 2
/// The Nursery within the supplied context is used as the test data setup area
pub fn setup_book_author_steps_with_context(
    context: &dyn HolonsContextBehavior,
    test_case: &mut DancesTestCase,
) -> Result<RelationshipName, HolonError> {
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.clone();

    //  STAGE:  Book Holon  //
    let mut book_holon = Holon::new();
    let book_holon_key = BOOK_KEY.clone();
    book_holon
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(book_holon_key.clone()),)?
        .with_property_value(
            PropertyName(MapString("title".to_string())),
            BaseValue::StringValue(book_holon_key.clone()),)?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    )?;
    test_case.add_stage_holon_step(book_holon.clone())?;

    let book_ref = stage_new_holon_api(context, book_holon)?;

    //  STAGE:  Person 1)  //
    let mut person_1_holon = Holon::new();
    let person_1_key = PERSON_1_KEY.clone();
    person_1_holon
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("Roger".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Briggs".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(person_1_key.clone()),
        )?;
    test_case.add_stage_holon_step(person_1_holon.clone())?;

    let person_1_reference = stage_new_holon_api(context, person_1_holon.clone())?;

    //  STAGE:  Person 2 Holon (H3)  //
    let mut person_2_holon = Holon::new();
    let person_2_key = PERSON_2_KEY.clone();
    person_2_holon
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("George".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Smith".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(person_2_key.clone()),
        )?;
    test_case.add_stage_holon_step(person_2_holon.clone())?;

    let person_2_reference = stage_new_holon_api(context, person_2_holon.clone())?;

    let mut publisher_holon = Holon::new();
    // assume publisher is at this position in new staged_holons vector
    let publisher_key = PUBLISHER_KEY.clone();
    publisher_holon
        .with_property_value(
            PropertyName(MapString("name".to_string())),
            BaseValue::StringValue(publisher_key.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(publisher_key.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString("We publish Holons for testing purposes".to_string())),
        )?;
    test_case.add_stage_holon_step(publisher_holon.clone())?;

    stage_new_holon_api(context, publisher_holon.clone())?;

    //  RELATIONSHIP:  (H1)-relationship_name>[(H2),(H3)]  //

    let mut target_references: Vec<HolonReference> = Vec::new();
    target_references.push(HolonReference::from_staged(person_1_reference));
    target_references.push(HolonReference::from_staged(person_2_reference));

    book_ref.add_related_holons(context, relationship_name.clone(), target_references.clone())?;

    // Create the expected_holon
    test_case.add_related_holons_step(
        book_ref.clone(), // source holon
        relationship_name.clone(),
        target_references,
        ResponseStatusCode::OK,
        book_ref.clone_holon(context)?,
    )?;

    Ok(relationship_name)
}
