use dances::dance_response::ResponseStatusCode;
use holons::{
    holon::Holon, holon_collection::HolonCollection, holon_error::HolonError,
    holon_reference::HolonReference, relationship::RelationshipName,
    smart_reference::SmartReference, staged_reference::StagedReference,
};
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapInteger, MapString, PropertyMap, PropertyName};

use crate::shared_test::test_data_types::{
    DanceTestState, DanceTestStep, DancesTestCase, TestReference,
};

use super::book_authors_setup_fixture::setup_book_author_steps;

/// Fixture for creating Simple StageNewFromClone Testcase
#[fixture]
pub fn simple_stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple StageNewFromClone Testcase".to_string(),
        "Tests stage_new_from_clone dance, creates and commits a holon, clones it, changes some properties, adds a relationship, commits it and then compares essential content of existing holon and cloned holon".to_string(),
    );
    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty except for HolonSpace  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));

    let test_data =
        setup_book_author_steps(&mut test_case, &mut holons_to_add, &desired_test_relationship)?;
    expected_count += test_data.len() as i64;

    // Get and set the various Holons data.
    let book_holon = test_data[0]
        .expected_holon
        .clone()
        .expect("Expected setup method to return Some book holon at index 0, got none.");
    let book_index = test_data[0].staged_index;
    let _book_key = test_data[0].key.clone();

    let publisher_index = test_data[3].staged_index;
    let publisher_key = test_data[3].key.clone();
    let publisher_holon_reference =
        HolonReference::Staged(StagedReference { holon_index: publisher_index.clone() });

    // CLONE A STAGED HOLON
    //  STAGE_NEW_FROM_CLONE -- StagedReference -- Book Holon Clone  //
    let cloned_book_index = 4;
    let cloned_book_key =
        BaseValue::StringValue(MapString("A clone from: Emerging World".to_string()));

    test_case.add_stage_new_from_clone_step(
        TestReference::StagedHolon(book_index),
        ResponseStatusCode::OK,
    )?;
    expected_count += 1;

    //  CHANGE PROPERTIES  //
    let mut properties = PropertyMap::new();
    properties.insert(PropertyName(MapString("title".to_string())), cloned_book_key.clone());
    properties.insert(PropertyName(MapString("key".to_string())), cloned_book_key.clone());
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString(
            "example property change for a clone from staged Holon".to_string(),
        )),
    );

    test_case.add_with_properties_step(
        cloned_book_index,
        properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // //  ADD RELATIONSHIP: Cloned Book -> Publisher  //
    let published_by_relationship_name = RelationshipName(MapString("PUBLISHED_BY".to_string()));
    let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
    // set expected
    let mut expected_book_holon = book_holon.clone();
    expected_book_holon.property_map = properties;
    let mut expected_publisher_holon_collection = HolonCollection::new_staged();
    expected_publisher_holon_collection
        .add_reference_with_key(Some(&publisher_key), &publisher_holon_reference)?;
    expected_book_holon
        .relationship_map
        .0
        .insert(published_by_relationship_name.clone(), expected_publisher_holon_collection);
    let expected_predecessor_holon_collection = HolonCollection::new_staged();
    expected_book_holon
        .relationship_map
        .0
        .insert(predecessor_relationship_name.clone(), expected_predecessor_holon_collection);

    test_case.add_related_holons_step(
        cloned_book_index, // source holon
        published_by_relationship_name.clone(),
        vec![publisher_holon_reference],
        ResponseStatusCode::OK,
        expected_book_holon.clone(), // expected holon
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT -- 5 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // // CLONE A SAVED HOLON
    // //  STAGE_NEW_FROM_CLONE -- SmartReference -- Book Holon Clone_2 //
    // let cloned_from_saved_book_index = 0;
    // let cloned_from_saved_book_key = BaseValue::StringValue(MapString(
    //     "A clone from the saved Holon: Emerging World".to_string(),
    // ));

    // test_case.add_stage_new_from_clone_step(
    //     TestReference::SavedHolon(book_key),
    //     ResponseStatusCode::OK,
    // )?;

    // //  CHANGE PROPERTIES  //
    // let mut changed_properties = PropertyMap::new();
    // changed_properties.insert(
    //     PropertyName(MapString("title".to_string())),
    //     cloned_from_saved_book_key.clone(),
    // );
    // changed_properties.insert(
    //     PropertyName(MapString("key".to_string())),
    //     cloned_from_saved_book_key,
    // );
    // changed_properties.insert(
    //     PropertyName(MapString("description".to_string())),
    //     BaseValue::StringValue(MapString("this is testing a clone from a saved Holon, changing it, modifying relationships, then committing".to_string())),
    // );

    // test_case.add_with_properties_step(
    //     cloned_from_saved_book_index,
    //     changed_properties.clone(),
    //     ResponseStatusCode::OK,
    // )?;

    // // STAGE:  A 2nd publisher Holon
    // // Another staged test Holon for adding a relationship to // temp workaround until support for passing TestReference to other test steps
    // let mut publisher_2_holon = Holon::new();
    // let publisher_2_index: usize = 1; // assume publisher is at this position in new staged_holons vector
    // let publisher_2_key = MapString("AnotherPublishingCompany".to_string());
    // let publisher_2_holon_reference = HolonReference::Staged(StagedReference {
    //     holon_index: publisher_2_index.clone(),
    // });
    // publisher_2_holon
    //     .with_property_value(
    //         PropertyName(MapString("name".to_string())),
    //         BaseValue::StringValue(MapString("Another Publishing Company".to_string())),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("key".to_string())),
    //         BaseValue::StringValue(publisher_2_key.clone()),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("description".to_string())),
    //         BaseValue::StringValue(MapString(
    //             "Again, We publish Holons for testing purposes".to_string(),
    //         )),
    //     )?;
    // test_case.add_stage_holon_step(publisher_2_holon.clone())?;

    // // //  ADD RELATIONSHIP: Cloned Saved Book Holon -> Publisher_2  //
    // // set expected

    // let mut expected_cloned_from_saved_book_holon = book_holon.clone();
    // expected_cloned_from_saved_book_holon.property_map = changed_properties;
    // let mut expected_publisher_2_holon_collection = HolonCollection::new_staged();
    // expected_publisher_2_holon_collection
    //     .add_reference_with_key(Some(&publisher_2_key), &publisher_2_holon_reference)?;
    // expected_cloned_from_saved_book_holon
    //     .relationship_map
    //     .0
    //     .insert(
    //         published_by_relationship_name.clone(),
    //         expected_publisher_2_holon_collection,
    //     );

    // test_case.add_related_holons_step(
    //     cloned_from_saved_book_index, // source holon
    //     published_by_relationship_name,
    //     vec![publisher_2_holon_reference],
    //     ResponseStatusCode::OK,
    //     expected_cloned_from_saved_book_holon.clone(), // expected holon
    // )?;

    // //  COMMIT  // the cloned & modified Book Holon
    // test_case.add_commit_step()?;

    // //  ENSURE DATABASE COUNT -- 7 Holons  //
    // test_case.add_ensure_database_count_step(MapInteger(7))?;

    // //  MATCH SAVED CONTENT //
    // test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
