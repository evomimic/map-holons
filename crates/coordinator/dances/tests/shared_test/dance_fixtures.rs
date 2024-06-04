// Simple Create Test Fixture
//
// This file is used to creates a TestCase that exercises the following steps:
// - Ensure database is empty
// - stage a new holon
// - update the staged holon's properties
// - commit the holon
// - get the holon
// - delete holon
// - ensure database is empty
//
//

#![allow(dead_code)]

use core::panic;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

use dances::dance_response::ResponseStatusCode;
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::context::HolonsContext;

use crate::shared_test::test_data_types::DancesTestCase;

// use hdk::prelude::*;

// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };

use holons::holon_error::HolonError;
use holons::relationship::RelationshipName;

use shared_types_holon::{
    MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_test_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure DB starts empty, stage Book and Person Holons, add properties, commit, ensure db count is 2".to_string(),

    );

    let mut expected_holons = Vec::new();

    test_case.add_ensure_database_count_step(MapInteger(0))?;

    let mut book_holon = Holon::new();
    book_holon.set_key_manually(MapString("Book Key".to_string()))?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;
    test_case.add_stage_holon_step(book_holon.clone())?;
    expected_holons.push(book_holon.clone());

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    );

    test_case.add_with_properties_step(0, properties, ResponseStatusCode::OK)?;

    let person_holon = Holon::new();

    test_case.add_stage_holon_step(person_holon.clone())?;
    expected_holons.push(person_holon.clone());

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(1, properties, ResponseStatusCode::OK)?;

    test_case.add_commit_step()?;
    test_case.add_match_saved_content_step()?;

    test_case.add_ensure_database_count_step(MapInteger(2))?;

    // test_case.holons = expected_holons;

    // let mut book_holon = Holon::new();
    // book_holon
    //     .with_property_value(
    //         PropertyName(MapString("title".to_string())),
    //         BaseValue::StringValue(MapString("Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string())))
    //     .with_property_value(
    //         PropertyName(MapString("description".to_string())),
    //         BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string())))
    //     ;
    // test_case.add_create_step(book_holon)?;

    // debug!("expected holons: {:?}", expected_holons);

    Ok(test_case.clone())
}
#[fixture]
pub fn simple_add_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple Add Related Holon Testcase".to_string(),
        "Ensure DB starts empty, stage Book and Person Holons, add properties, commit, ensure db count is 2".to_string(),

    );

    test_case.add_ensure_database_count_step(MapInteger(0))?;

    let mut book_holon = Holon::new();
    book_holon.set_key_manually(MapString("Book Key".to_string()))?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;
    test_case.add_stage_holon_step(book_holon)?;
    let book_index: StagedIndex = 0;

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    );

    test_case.add_with_properties_step(book_index, properties, ResponseStatusCode::OK)?;

    let person_holon_briggs = Holon::new();

    test_case.add_stage_holon_step(person_holon_briggs)?;
    let briggs_index: StagedIndex = 1;
    let briggs_staged_reference = StagedReference {
        holon_index: briggs_index,
    };
    let briggs_holon_reference = HolonReference::Staged(briggs_staged_reference);

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(briggs_index, properties, ResponseStatusCode::OK)?;

    let mut person_holon_gebser = Holon::new();
    person_holon_gebser
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("Jean".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Gebser".to_string())),
        )?;
    test_case.add_stage_holon_step(person_holon_gebser)?;
    let gebser_index: StagedIndex = 2;
    let gebser_staged_reference = StagedReference {
        holon_index: gebser_index,
    };
    let gebser_reference = HolonReference::Staged(gebser_staged_reference);

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    holons_to_add.push(briggs_holon_reference);
    holons_to_add.push(gebser_reference);

    test_case.add_related_holons_step(
        book_index,
        RelationshipName(MapString("AUTHORS".to_string())),
        holons_to_add,
        ResponseStatusCode::OK,
    )?;

    test_case.add_commit_step()?;

    test_case.add_ensure_database_count_step(MapInteger(3))?;

    Ok(test_case.clone())
}

/// Fixture for creating Simple AbandonStagedChanges Testcase
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple AbandonStagedChanges Testcase".to_string(),
        "Tests abandon_staged_changes dance, confirms behavior of commit and verifies abandoned holon is not accessible".to_string(),
    );

    test_case.add_ensure_database_count_step(MapInteger(0))?;
    //
    // H1, H2, H3, etc refer to order of Holons added to staging area.
    // Before the commit process, these Holons are only able to be identified by their index in the staging_area Vec,
    // therefore it is necessary to maintain their order.
    // Each Holon's index can be figured by subtracting 1. Ex H1 is index 0, H2 index 1
    //
    //

    //  ADD STEP:  STAGE:  Book Holon (H1)  //
    let mut book = Holon::new();
    book.set_key_manually(MapString("Book Key".to_string()))?;
    book.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?.with_property_value(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    )?;
    test_case.add_stage_holon_step(book.clone())?;
    let book_index: usize = 0; // assume book is at this position in staged_holons vector

    //  ADD STEP:  STAGE:  Person 1 Holon (H2)  //
    let mut person_1 = Holon::new();
    person_1
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("Roger".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Briggs".to_string())),
        )?;
    test_case.add_stage_holon_step(person_1.clone())?;
    let person_1_index: usize = 1; // assume person_1 is at this position in staged_holons vector
    let person_1_staged_reference = StagedReference {
        holon_index: person_1_index,
    };

    //  ADD STEP:  STAGE:  Person 2 Holon (H3)  //
    let mut person_holon_2 = Holon::new();
    person_holon_2
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("George".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Smith".to_string())),
        )?;
    test_case.add_stage_holon_step(person_holon_2.clone())?;
    let person_2_index: usize = 2; // assume person_1 is at this position in staged_holons vector
    let person_2_staged_reference = StagedReference {
        holon_index: person_2_index,
    };

    // ADD STEP:  RELATIONSHIP:  Book H1-> Author H2 & H3  //
    test_case.add_related_holons_step(
        book_index, // source holon
        RelationshipName(MapString("AUTHORED_BY".to_string())),
        vec![
            HolonReference::Staged(person_1_staged_reference.clone()),
            HolonReference::Staged(person_2_staged_reference.clone()),
        ],
        ResponseStatusCode::OK,
    )?;

    // ADD STEP:  ABANDON:  H2
    // This step verifies the abandon dance succeeds and that subsequent operations on the
    // abandoned Holon return NotAccessible Errors
    test_case.add_abandon_staged_changes_step(person_1_index, ResponseStatusCode::OK)?;

    // ADD STEP:  RELATIONSHIP:  Author H2 -> H3  //
    // Attempt add_related_holon dance -- expect Conflict/NotAccessible response
    test_case.add_related_holons_step(
        person_1_index, // source holons
        RelationshipName(MapString("FRIENDS".to_string())),
        vec![
            HolonReference::Staged(person_1_staged_reference),
            HolonReference::Staged(person_2_staged_reference),
        ],
        ResponseStatusCode::Conflict,
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(2))?;

    // ADD STEP:  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    //  ADD STEP:  STAGE:  Abandoned Holon1 (H4)  //
    let mut abandoned_holon_1 = Holon::new();
    abandoned_holon_1.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test2".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_1.clone())?;

    //  ADD STEP:  STAGE:  Abandoned Holon2 (H5)  //
    let mut abandoned_holon_2 = Holon::new();
    abandoned_holon_2.with_property_value(
        PropertyName(MapString("example abandon".to_string())),
        BaseValue::StringValue(MapString("test2".to_string())),
    )?;
    test_case.add_stage_holon_step(abandoned_holon_2.clone())?;

    // ADD STEP:  ABANDON:  H4
    test_case.add_abandon_staged_changes_step(0, ResponseStatusCode::OK)?;

    // ADD STEP:  ABANDON:  H5
    test_case.add_abandon_staged_changes_step(1, ResponseStatusCode::OK)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(2))?;

    // ADD STEP:  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
