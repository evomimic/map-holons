use dances::{dance_response::ResponseStatusCode, holon_dance_adapter::QueryExpression};
use hdi::prelude::warn;
use holons::{
    holon::Holon, holon_collection::HolonCollection, holon_error::HolonError,
    holon_reference::HolonReference, relationship::RelationshipName,
    smart_reference::SmartReference, staged_reference::StagedReference,
};
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapInteger, MapString, PropertyMap, PropertyName};

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

    let book_holon = test_data[0]
        .expected_holon
        .clone()
        .expect("Expected setup method to return Some book holon at index 0, got none.");

    let person_1_index = test_data[1].staged_index;
    let person_1_key = test_data[1].key.clone();
    let person_1_holon_reference = HolonReference::Staged(StagedReference {
        holon_index: person_1_index.clone(),
    });

    let person_2_index = test_data[2].staged_index;
    let person_2_key = test_data[2].key.clone();
    let person_2_holon_reference = HolonReference::Staged(StagedReference {
        holon_index: person_2_index.clone(),
    });

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

    //  STAGE_NEW_FROM_CLONE -- StagedReference -- Book Holon Clone  //
    let mut cloned_book_holon = book_holon.clone();
    let cloned_book_index = 4;
    let cloned_book_key =
        BaseValue::StringValue(MapString("A clone from: Emerging World".to_string()));

    //  CHANGE PROPERTIES  //
    cloned_book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        cloned_book_key.clone(),
    )?;
    cloned_book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        cloned_book_key.clone(),
    )?;
    cloned_book_holon.with_property_value(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString(
            "example property change for a clone from staged Holon".to_string(),
        )),
    )?;

    test_case.add_stage_new_from_clone_step(
        book_holon.clone(),
        ResponseStatusCode::OK,
        cloned_book_holon.clone(),
    )?;

    // //  REMOVE RELATIONSHIP: Book -> Person_1  //
    let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
    // set expected
    cloned_book_holon.relationship_map.0.insert(
        predecessor_relationship_name.clone(),
        HolonCollection::new_staged(),
    );
    let mut expected_authored_by_holon_collection = HolonCollection::new_staged();
    expected_authored_by_holon_collection
        .add_reference_with_key(Some(&person_2_key), &person_2_holon_reference)?;
    cloned_book_holon.relationship_map.0.insert(
        desired_test_relationship.clone(),
        expected_authored_by_holon_collection,
    );

    test_case.remove_related_holons_step(
        cloned_book_index, // source holon
        desired_test_relationship.clone(),
        vec![HolonReference::Staged(StagedReference::new(person_1_index))], // removing person_1
        ResponseStatusCode::OK,
        cloned_book_holon.clone(), // expected holon
    )?;

    // //  ADD RELATIONSHIP: Book -> Publisher  //
    let published_by_relationship_name = RelationshipName(MapString("PUBLISHED_BY".to_string()));
    // set expected
    let mut expected_publisher_holon_collection = HolonCollection::new_staged();
    expected_publisher_holon_collection
        .add_reference_with_key(Some(&publisher_key), &publisher_holon_reference)?;
    cloned_book_holon.relationship_map.0.insert(
        published_by_relationship_name.clone(),
        expected_publisher_holon_collection,
    );

    test_case.add_related_holons_step(
        cloned_book_index, // source holon
        published_by_relationship_name,
        vec![publisher_holon_reference.clone()],
        ResponseStatusCode::OK,
        cloned_book_holon.clone(), // expected holon
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT -- 5 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(5))?;

    //  MATCH SAVED CONTENT -- PASS 1 -- Pre-modification  //
    test_case.add_match_saved_content_step()?;

    //TODO:
    // CLONE A SAVED HOLON
    // //  STAGE_NEW_FROM_CLONE -- SmartReference -- Book Holon Clone_2 //
    // let mut cloned_from_saved_book_holon: Holon = book_holon.clone();
    // let cloned_from_saved_book_index = 5;
    // let cloned_from_saved_book_key = BaseValue::StringValue(MapString(
    //     "A clone from the saved Holon: Emerging World".to_string(),
    // ));

    // //  CHANGE PROPERTIES  //
    // cloned_from_saved_book_holon.with_property_value(
    //     PropertyName(MapString("title".to_string())),
    //     cloned_book_key.clone(),
    // )?;
    // cloned_from_saved_book_holon.with_property_value(
    //     PropertyName(MapString("key".to_string())),
    //     cloned_from_saved_book_key,
    // )?;
    // cloned_from_saved_book_holon.with_property_value(
    //     PropertyName(MapString("description".to_string())),
    //     BaseValue::StringValue(MapString("this is testing a clone from a saved Holon, changing it, modifying relationships, then committing".to_string())),
    // )?;

    // test_case.add_stage_new_from_clone_step(
    //     book_holon.clone(),
    //     ResponseStatusCode::OK,
    //     cloned_from_saved_book_holon.clone(),
    // )?;

    // // //  REMOVE RELATIONSHIP: Book -> Person_2  //
    // let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
    // // set expected
    // cloned_from_saved_book_holon.relationship_map.0.insert(
    //     predecessor_relationship_name.clone(),
    //     HolonCollection::new_staged(),
    // );
    // let mut expected_authored_by_holon_collection = HolonCollection::new_staged();
    // expected_authored_by_holon_collection
    //     .add_reference_with_key(Some(&person_1_key), &person_1_holon_reference)?;
    // cloned_from_saved_book_holon.relationship_map.0.insert(
    //     desired_test_relationship.clone(),
    //     expected_authored_by_holon_collection,
    // );

    // test_case.remove_related_holons_step(
    //     cloned_from_saved_book_index, // source holon
    //     desired_test_relationship.clone(),
    //     vec![HolonReference::Smart(SmartReference::new(
    //         HolonId::Local(cloned_from_saved_holon_id),
    //         Some(cloned_from_saved_book_holon.property_map.clone()),
    //     ))], // removing person_2
    //     ResponseStatusCode::OK,
    //     cloned_from_saved_book_holon.clone(), // expected holon
    // )?;

    // // //  ADD RELATIONSHIP: Book -> Publisher  //
    // let published_by_relationship_name = RelationshipName(MapString("PUBLISHED_BY".to_string()));
    // // set expected
    // let mut expected_publisher_holon_collection = HolonCollection::new_staged();
    // expected_publisher_holon_collection
    //     .add_reference_with_key(Some(&publisher_key), &publisher_holon_reference)?;
    // cloned_from_saved_book_holon.relationship_map.0.insert(
    //     published_by_relationship_name.clone(),
    //     expected_publisher_holon_collection,
    // );

    // test_case.add_related_holons_step(
    //     cloned_from_saved_book_index, // source holon
    //     published_by_relationship_name,
    //     vec![publisher_holon_reference],
    //     ResponseStatusCode::OK,
    //     cloned_from_saved_book_holon.clone(), // expected holon
    // )?;

    // //  COMMIT  // the cloned & modified Book Holon
    // test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT -- 6 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(6))?;

    //  MATCH SAVED CONTENT -- PASS 2 -- Post-modification  //
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
