#![allow(dead_code)]

use crate::get_holon_by_key_from_test_state;
use crate::tracing::{error, info, warn};
use core::panic;
//use holochain::core::author_key_is_valid;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_collection::{CollectionState, HolonCollection};
use holons::holon_reference::HolonReference;
use holons::query::QueryExpression;
use holons::smart_reference::SmartReference;
use holons::staged_reference::StagedReference;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

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
use holons::holon_reference::HolonReference::Staged;
use holons::relationship::RelationshipName;

use crate::shared_test::book_authors_setup_fixture::setup_book_author_steps;
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};

use super::book_authors_setup_fixture::setup_book_author_steps;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_test_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure DB starts empty, stage Book and Person Holons, add properties, commit, ensure db count is 2".to_string(),

    );

    let mut expected_holons = Vec::new();
    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count:i64  = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Create book Holon with properties //
    let mut book_holon = Holon::new();
    book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;

    //  STAGE:  Book Holon  //
    test_case.add_stage_holon_step(book_holon.clone())?;
    expected_holons.push(book_holon.clone());
    expected_count += 1;

    //  PROPERTIES:  Book  //
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    );
    test_case.add_with_properties_step(0, properties, ResponseStatusCode::OK)?;

    //  STAGE:  Person Holon  //
    let person_holon = Holon::new();
    test_case.add_stage_holon_step(person_holon.clone())?;
    expected_holons.push(person_holon.clone());
    expected_count += 1;

    //  PROPERTIES:  Person  //
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("RogerBriggs".to_string())),
    );
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(1, properties, ResponseStatusCode::OK)?;

    //  COMMIT  //
    test_case.add_commit_step()?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    Ok(test_case.clone())
}

#[fixture]
pub fn simple_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple Add Related Holon Testcase".to_string(),
        "Ensure DB starts empty, stage Book and Person Holons, add properties, commit, ensure db count is 3".to_string(),

    );

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count:i64  = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;


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
    expected_count += 1;

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
    let person_1_reference = Staged(StagedReference {
        holon_index: person_1_index,
    });
    expected_count += 1;

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
    let person_2_reference = Staged(StagedReference {
        holon_index: person_2_index,
    });
    expected_count += 1;

    //  RELATIONSHIP:  Book H1-> Author H2 & H3  //

    let authored_by_relationship_name = RelationshipName(MapString("AUTHORED_BY".to_string()));

    // Create the expected_holon
    let mut authored_by_collection = HolonCollection::new_staged();
    authored_by_collection
        .add_reference_with_key(Some(&person_1_key), &person_1_reference.clone())?;

    authored_by_collection.add_reference_with_key(Some(&person_2_key), &person_2_reference)?;

    book_holon.relationship_map.0.insert(
        authored_by_relationship_name.clone(),
        authored_by_collection.clone(),
    );

    let mut related_holons: Vec<HolonReference> = Vec::new();
    related_holons.push(person_1_reference.clone());
    related_holons.push(person_2_reference);

    test_case.add_related_holons_step(
        book_index, // source holon
        authored_by_relationship_name.clone(),
        related_holons.to_vec(),
        ResponseStatusCode::OK,
        book_holon.clone(),
    )?;

    let empty_collection = HolonCollection::new_staged();

    let mut book_holon_with_no_related = book_holon.clone();
    book_holon_with_no_related.relationship_map.0.clear();
    book_holon_with_no_related
        .relationship_map
        .0
        .insert(authored_by_relationship_name.clone(), empty_collection);

    let mut one_in_collection = HolonCollection::new_staged();
    one_in_collection.add_reference_with_key(Some(&person_1_key), &person_1_reference)?;

    let mut book_holon_with_one_related = book_holon.clone();
    book_holon_with_one_related.relationship_map.0.clear();
    book_holon_with_one_related
        .relationship_map
        .0
        .insert(authored_by_relationship_name.clone(), one_in_collection);

    // // test invalid source holon
    let wrong_book_index: usize = 8;
    // the cache manager returns a IndexOutOfRange ServerError .. not a Notfound 404
    test_case.remove_related_holons_step(
        wrong_book_index, // source holon
        authored_by_relationship_name.clone(),
        related_holons.to_vec(),
        ResponseStatusCode::ServerError,
        book_holon.clone(), //expected
    )?;

    // test invalid relationship name
    let wrong_relationship_name: RelationshipName = RelationshipName(MapString("WRONG".into()));
    test_case.remove_related_holons_step(
        book_index, // source holon
        wrong_relationship_name,
        related_holons.to_vec(),
        ResponseStatusCode::BadRequest,
        book_holon.clone(), //expected
    )?;

    // test remove one related holon
    test_case.remove_related_holons_step(
        book_index, // source holon
        authored_by_relationship_name.clone(),
        related_holons.clone().split_off(1), //takes the second person holon
        ResponseStatusCode::OK,
        book_holon_with_one_related.clone(), //expected
    )?;

    // test remove all related holons including ignoring a previous one that was already removed
    test_case.remove_related_holons_step(
        book_index, // source holon
        authored_by_relationship_name.clone(),
        related_holons.to_vec(),
        ResponseStatusCode::OK,
        book_holon_with_no_related.clone(), //expected none
    )?;

    test_case.add_related_holons_step(
        book_index, // source holon
        authored_by_relationship_name.clone(),
        related_holons.to_vec(),
        ResponseStatusCode::OK,
        book_holon.clone(),
    )?;

    //  COMMIT  //
    test_case.add_commit_step()?;
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;


    //  QUERY RELATIONSHIPS  //
    let query_expression = QueryExpression::new(authored_by_relationship_name.clone());
    test_case.add_query_relationships_step(
        book_holon_key,
        query_expression,
        ResponseStatusCode::OK,
    )?;

    Ok(test_case.clone())
}

/// Fixture for creating Simple AbandonStagedChanges Testcase
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple AbandonStagedChanges Testcase".to_string(),
        "Tests abandon_staged_changes dance, confirms behavior of commit and verifies abandoned holon is not accessible".to_string(),
    );

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count:i64  = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));
    let test_data = setup_book_author_steps(
        &mut test_case,
        &mut holons_to_add,
        &desired_test_relationship,
    )?;
    expected_count += test_data.len() as i64;

    let person_1_index = test_data[1].staged_index;
    let book_holon_key = test_data[0].key.clone();
    let book_holon = test_data[0]
        .expected_holon
        .clone()
        .expect("Expected setup method to return Some book holon at index 0, got none.");

    //  ABANDON:  H2  //
    // This step verifies the abandon dance succeeds and that subsequent operations on the
    // abandoned Holon return NotAccessible Errors
    test_case.add_abandon_staged_changes_step(person_1_index, ResponseStatusCode::OK)?;
    expected_count -= 1;

    //  RELATIONSHIP:  Author H2 -> H3  //
    // Attempt add_related_holon dance -- expect Conflict/NotAccessible response
    test_case.add_related_holons_step(
        person_1_index, // source holons
        RelationshipName(MapString("FRIENDS".to_string())),
        holons_to_add.to_vec(),
        ResponseStatusCode::Conflict,
        book_holon.clone(),
    )?;

    //  COMMIT  //  all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    //  STAGE:  Abandoned Holon1 (H4)  //
    let mut abandoned_holon_1 = Holon::new();
    abandoned_holon_1.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("Abandon1".to_string())),
    )?;
    abandoned_holon_1.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test1".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_1.clone())?;
    expected_count += 1;

    //  STAGE:  Abandoned Holon2 (H5)  //
    let mut abandoned_holon_2 = Holon::new();
    abandoned_holon_2.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("Abandon2".to_string())),
    )?;
    abandoned_holon_2.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test2".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_2.clone())?;
    expected_count += 1;

    // ABANDON:  H4
    test_case.add_abandon_staged_changes_step(0, ResponseStatusCode::OK)?;
    expected_count -= 1;

    // ABANDON:  H5
    test_case.add_abandon_staged_changes_step(1, ResponseStatusCode::OK)?;
    expected_count -= 1;

    // COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    // ADD STEP: QUERY RELATIONSHIPS //
    let query_expression = QueryExpression::new(desired_test_relationship.clone());
    test_case.add_query_relationships_step(
        book_holon_key,
        query_expression,
        ResponseStatusCode::OK,
    )?;

    Ok(test_case.clone())
}

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    //  ADD STEP:  STAGE:  Book Holon  //
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
    test_case.add_stage_holon_step(book_holon)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(ResponseStatusCode::OK)?;

    // ADD STEP: DELETE HOLON - Invalid //
    test_case.add_delete_holon_step(ResponseStatusCode::NotFound)?;

    Ok(test_case.clone())
}
