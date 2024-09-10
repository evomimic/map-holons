

#![allow(dead_code)]

use crate::get_holon_by_key_from_test_state;
use crate::tracing::{error, info, warn};
use core::panic;
use dances::holon_dance_adapter::{Node, NodeCollection, QueryExpression};
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_collection::{CollectionState, HolonCollection};
use holons::holon_reference::HolonReference;
use holons::smart_reference::SmartReference;
use holons::staged_reference::StagedReference;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;
use holochain::core::author_key_is_valid;

use dances::dance_response::ResponseStatusCode;
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::context::HolonsContext;

use crate::shared_test::data_types::DancesTestCase;

// use hdk::prelude::*;

// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };

use holons::holon_error::HolonError;
use holons::relationship::RelationshipName;

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
/// *Person 1 Holon* whose key is "RogerBriggs"
/// *Person 2 Holon* whose key is "GeorgeSmith"
/// An *AUTHORED_BY* relationship from the Book Holon to both Person Holon's
/// The HolonReference and key are returned for each holon via the Vec<TestHolon> result
pub fn setup_book_author_steps(
    test_case:&mut DancesTestCase,
    holons_to_add: &mut Vec<HolonReference>,
    relationship_name: &RelationshipName)
    -> Result<Vec<TestHolon>, HolonError> {

    let mut result : Vec<TestHolon>= Vec::new();

    //
    // H1, H2, H3, etc. refer to order of Holons added to staging area.
    // Before the commit process, these Holons are identified by their index in the staging_area Vec,
    // therefore it is necessary to maintain their order.
    // Each Holon's index can be figured by subtracting 1. Ex H1 is index 0, H2 index 1
    //
    //


    //  STAGE:  Book Holon (H1)  //
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
    test_case.add_stage_holon_step(book_holon.clone())?;

    let book_index: usize = 0; // assume book is at this position in staged_holons vector

    //  STAGE:  Person 1 Holon (H2)  //
    let mut person_1 = Holon::new();
    let person_1_key = MapString("RogerBriggs".to_string());
    person_1
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
    test_case.add_stage_holon_step(person_1.clone())?;

    let person_1_index: usize = 1; // assume person_1 is at this position in staged_holons vector
    let person_1_reference = HolonReference::Staged(StagedReference {holon_index: person_1_index,});


    //  STAGE:  Person 2 Holon (H3)  //
    let mut person_holon_2 = Holon::new();
    let person_2_key = MapString("GeorgeSmith".to_string());
    person_holon_2
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
    test_case.add_stage_holon_step(person_holon_2.clone())?;
    let person_2_index: usize = 2; // assume person_1 is at this position in staged_holons vector
    let person_2_reference = HolonReference::Staged(StagedReference {holon_index: person_2_index,});


    //  RELATIONSHIP:  (H1)-relationship_name>[(H2),(H3)]  //


    // Create the expected_holon
    let mut target_collection = HolonCollection::new_staged();
    target_collection.add_reference_with_key(
        Some(&person_1_key),
        &person_1_reference)?;

    target_collection.add_reference_with_key(
        Some(&person_2_key),
        &person_2_reference)?;


    book_holon.relationship_map.0.insert(
        relationship_name.clone(),
        target_collection,
    );

    // let mut holons_to_add: Vec<HolonReference> = Vec::new();
    holons_to_add.push(person_1_reference);
    holons_to_add.push(person_2_reference);


    test_case.add_related_holons_step(
        book_index, // source holon
        relationship_name.clone(),
        holons_to_add.to_vec(),
        ResponseStatusCode::OK,
        book_holon.clone(),
    )?;

    result.push(TestHolon {
        staged_index:book_index,
        key: book_holon_key.clone(),
        expected_holon: Some(book_holon.clone())
    });
    result.push(TestHolon {
        staged_index: person_1_index,
        key: person_1_key.clone(),
        expected_holon: None
    });

    result.push(TestHolon {
        staged_index: person_2_index,
        key: person_2_key.clone(),
        expected_holon: None }
    );

    Ok(result)
}


