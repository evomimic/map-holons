use dances::{dance_response::ResponseStatusCode, holon_dance_adapter::QueryExpression};
use hdi::prelude::warn;
use holons::{
    holon::Holon, holon_collection::HolonCollection, holon_error::HolonError,
    holon_reference::HolonReference, relationship::RelationshipName,
    staged_reference::StagedReference,
};
use rstest::*;
use shared_types_holon::{BaseValue, MapInteger, MapString, PropertyMap, PropertyName};

use crate::data_types::DancesTestCase;

use super::book_authors_setup_fixture::setup_book_author_steps;

/// Fixture for creating Simple StageNewFromClone Testcase
#[fixture]
pub fn simple_stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple StageNewFromClone Testcase".to_string(),
        "Tests stage_new_from_clone dance, creates and commits a holon, clones it, changes some properties, adds and removes some relationships, commits it and then compares essential content of existing holon and cloned holon".to_string(),
    );

    //  ENSURE DATABASE COUNT -- Empty  //
    test_case.add_ensure_database_count_step(MapInteger(0))?;

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, and an AUTHORED_BY relationship from
    // the book to both persons.
    let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));
    let test_data = setup_book_author_steps(
        &mut test_case,
        &mut holons_to_add,
        &desired_test_relationship,
    )?;

    let book_index = test_data[0].staged_index;
    let book_holon_key = test_data[0].key.clone();
    let book_holon = test_data[0]
        .expected_holon
        .clone()
        .expect("Expected setup method to return Some book holon at index 0, got none.");

    let person_1_index = test_data[1].staged_index;
    let person_2_index = test_data[2].staged_index;

    // //  STAGE:  Publisher Holon  //
    // An additional Holon for adding relationships to.

    let mut publisher_holon = Holon::new();
    let publisher_index: usize = 3; // assume pubsliher is at this position in new staged_holons vector
    let publisher_holon_reference = HolonReference::Staged(StagedReference {
        holon_index: publisher_index,
    });
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
            BaseValue::StringValue(MapString(
                "We publish Holons for testing purposes".to_string(),
            )),
        )?;

    test_case.add_stage_holon_step(publisher_holon.clone())?;

    // //  ADD RELATIONSHIP: Book -> Publisher  //

    // test_case.add_related_holons_step(
    //     book_index, // source holon
    //     RelationshipName(MapString("PUBLISHED_BY".to_string())),
    //     vec![publisher_holon_reference],
    //     ResponseStatusCode::OK,
    //     book_holon.clone(),
    // )?;

    //  STAGE_NEW_FROM_CLONE -- StagedReference -- Book Holon Clone  //
    let mut cloned_book_holon = Holon::new();
    let cloned_book_index = 4;
    let cloned_book_key =
        BaseValue::StringValue(MapString("A clone from: Emerging World".to_string()));

    //  CHANGE PROPERTIES  //
    cloned_book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        cloned_book_key.clone(),
    )?;
    cloned_book_holon
        .with_property_value(PropertyName(MapString("key".to_string())), cloned_book_key)?;
    cloned_book_holon.with_property_value(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("example property change".to_string())),
    )?;

    // test_data.push(TestHolon { staged_index: cloned_book_index, key: cloned_book_key, expected_holon: Some(cloned_book)});

    test_case.add_stage_new_from_clone_step(
        book_holon.clone(),
        ResponseStatusCode::OK,
        cloned_book_holon,
    )?;

    // //  REMOVE RELATIONSHIP: Book -> Person_1  //
    // test_case.remove_related_holons_step(
    //     book_index, // source holon
    //     desired_test_relationship.clone(),
    //     vec![HolonReference::Smart(SmartReference {
    //         holon_id: //?,
    //     })],
    //     ResponseStatusCode::OK,
    //     book_holon.clone(),
    // )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT -- 5 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(5))?;

    //  MATCH SAVED CONTENT -- PASS 1 -- Pre-modification  //
    test_case.add_match_saved_content_step()?;

    // CLONE A SAVED HOLON

    // add a step to

    // // let mut cloned_book = book_holon.clone();
    // let cloned_book_index = 3;
    // let cloned_book_key =
    //     BaseValue::StringValue(MapString("A clone from: Emerging World".to_string()));
    // //  CHANGE PROPERTIES  //
    // let mut properties = PropertyMap::new();
    // properties.insert(
    //     PropertyName(MapString("title".to_string())),
    //     cloned_book_key.clone(),
    // );
    // properties.insert(PropertyName(MapString("key".to_string())), cloned_book_key);
    // properties.insert(
    //     PropertyName(MapString("description".to_string())),
    //     BaseValue::StringValue(MapString("example property change".to_string())),
    // );
    // // cloned_book.property_map = properties.clone();
    // // test_data.push(TestHolon { staged_index: cloned_book_index, key: cloned_book_key, expected_holon: Some(cloned_book)});

    // test_case.add_with_properties_step(cloned_book_index, properties, ResponseStatusCode::OK)?;

    // //  REMOVE RELATIONSHIP: Book -> Person_2  //
    // test_case.remove_related_holons_step(
    //     book_index, // source holon
    //     desired_test_relationship.clone(),
    //     vec![HolonReference::Smart(SmartReference {
    //         holon_id: //?,
    //     })],
    //     ResponseStatusCode::OK,
    //     book_holon.clone(),
    // )?;

    /*
    // //  STAGE:  Publisher Holon  //
    // let mut publisher_holon = Holon::new();
    // let publisher_index: usize = 1; // assume pubsliher is at this position in new staged_holons vector
    // let publisher_holon_reference = HolonReference::Staged(StagedReference {
    //     holon_index: publisher_index,
    // });
    // let publisher_key = MapString("Publishing Company".to_string());
    // publisher_holon
    //     .with_property_value(
    //         PropertyName(MapString("name".to_string())),
    //         BaseValue::StringValue(MapString("Publishing Company".to_string())),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("key".to_string())),
    //         BaseValue::StringValue(publisher_key.clone()),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("description".to_string())),
    //         BaseValue::StringValue(MapString(
    //             "We publish Holons for testing purposes".to_string(),
    //         )),
    //     )?;

    // test_case.add_stage_holon_step(publisher_holon.clone())?;

    // //  ADD RELATIONSHIP: Book -> Publisher  //

    // test_case.add_related_holons_step(
    //     book_index, // source holon
    //     RelationshipName(MapString("PUBLISHED_BY".to_string())),
    //     vec![publisher_holon_reference],
    //     ResponseStatusCode::OK,
    //     book_holon.clone(),
    // )?;
    */

    // //  COMMIT  // the cloned & modified Book Holon
    // test_case.add_commit_step()?;

    // //  ENSURE DATABASE COUNT -- 4 Holons  //
    // test_case.add_ensure_database_count_step(MapInteger(4))?;

    // //  MATCH SAVED CONTENT -- PASS 2 -- Post-modification  //
    // test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
