#![allow(dead_code)]

use core::panic;
use holochain::core::author_key_is_valid;

use holons::HolonCollectionApi;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

use crate::shared_test::test_data_types::DancesTestCase;
use crate::test_extensions::TestContextExtensions; // Import the test-only extension

use dances::dance_response::ResponseStatusCode;
use holons::reference_layer::staged_reference::StagedIndex;
use holons::reference_layer::{HolonReference, StagedReference};
use holons_core::core_shared_objects::holon_pool::HolonPool;
use holons_core::core_shared_objects::{
    Holon, HolonCollection, HolonError, RelationshipName, TransientCollection,
};
use holons_core::{stage_new_holon_api, HolonWritable, HolonsContextBehavior};
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};

pub struct TestHolon {
    pub staged_index: StagedIndex,
    pub key: MapString,
    pub expected_holon: Option<Holon>,
}
/// This function updates the supplied test_case with a set of steps that establish some basic
/// data the different test cases can then extend for different purposes.
/// Specifically, this function stages (but does NOT commit) the following test data:
/// *Book Holon* whose title and key are "Emerging World: The Evolution of Consciousness and the Future of Humanity"
/// *Person 1 Holon* whose key is "Roger Briggs"
/// *Person 2 Holon* whose key is "George Smith"
/// *Publisher Holon* whose key is "Publishing Company"
/// An *AUTHORED_BY* relationship from the Book Holon to both Person Holon's
/// The HolonReference and key are returned for each holon via the HolonPool result
///
/// THe relationship_name used in this method is returned in the result to ensure consistency between
/// this function and the caller.
pub fn setup_book_author_steps_with_context(
    context: &dyn HolonsContextBehavior,
    test_case: &mut DancesTestCase,
) -> Result<RelationshipName, HolonError> {
    let relationship_name = RelationshipName(MapString("AUTHORED_BY".to_string()));

    //  STAGE:  Book Holon  //
    let mut book_holon = Holon::new();
    let book_holon_key = MapString(
        "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
    );
    book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(book_holon_key.clone()),
    )?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?.with_property_value(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    )?;
    test_case.add_stage_holon_step(book_holon.clone(), true)?;

    let book_ref = stage_new_holon_api(context, book_holon.clone())?;
    // result
    //     .add_reference_with_key(Some(&book_holon_key), &HolonReference::Staged(book_ref.clone()))?;

    let book_index: usize = 0; // assume book is at this position in staged_holons vector

    //  STAGE:  Person "Roger Briggs")  //
    let mut person_1_holon = Holon::new();
    let person_1_key = MapString("Roger Briggs".to_string());
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
    test_case.add_stage_holon_step(person_1_holon.clone(), true)?;

    let person_1_reference = stage_new_holon_api(context, person_1_holon.clone())?;

    // let person_1_reference =
    //     HolonReference::Staged(stage_new_holon_api(context, person_1_holon.clone())?);
    // result.add_reference_with_key(Some(&person_1_key), &person_1_reference.clone())?;

    // let person_1_index: usize = 1; // assume person_1 is at this position in staged_holons vector
    // let person_1_reference =
    //     HolonReference::Staged(StagedReference { holon_index: person_1_index });

    //  STAGE:  Person 2 Holon (H3)  //
    let mut person_2_holon = Holon::new();
    let person_2_key = MapString("George Smith".to_string());
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
    test_case.add_stage_holon_step(person_2_holon.clone(), true)?;

    let person_2_reference = stage_new_holon_api(context, person_2_holon.clone())?;

    // let person_2_index: usize = 2; // assume person_1 is at this position in staged_holons vector
    // let person_2_reference =
    //     HolonReference::Staged(StagedReference { holon_index: person_2_index });

    //  STAGE:  Publisher Holon (H4)  //
    let mut publisher_holon = Holon::new();
    // assume publisher is at this position in new staged_holons vector
    let publisher_key = MapString("Publishing Company".to_string());
    publisher_holon
        .with_property_value(
            PropertyName(MapString("name".to_string())),
            BaseValue::StringValue(MapString("Publishing Company".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(publisher_key.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString("We publish Holons for testing purposes".to_string())),
        )?;
    test_case.add_stage_holon_step(publisher_holon.clone(), true)?;

    stage_new_holon_api(context, publisher_holon.clone())?;

    //  RELATIONSHIP:  (H1)-relationship_name>[(H2),(H3)]  //

    let mut target_references: Vec<HolonReference> = Vec::new();
    target_references.push(HolonReference::from_staged(person_1_reference));
    target_references.push(HolonReference::from_staged(person_2_reference));

    book_ref.add_related_holons(context, relationship_name.clone(), target_references.clone())?;

    // Create the expected_holon
    test_case.add_related_holons_step(
        StagedReference::from_index(book_index), // source holon
        relationship_name.clone(),
        target_references,
        ResponseStatusCode::OK,
        book_holon.clone(),
    )?;

    Ok(relationship_name)
}
