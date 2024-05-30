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
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

use dances::dance_request::PortableReference;
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

    test_case.add_with_properties_step(0, properties)?;

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
    test_case.add_with_properties_step(1, properties)?;

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

    test_case.add_with_properties_step(book_index, properties)?;

    let person_holon_briggs = Holon::new();

    test_case.add_stage_holon_step(person_holon_briggs)?;
    let briggs_index: StagedIndex = 1;
    let briggs_reference = PortableReference::Staged(briggs_index);

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(briggs_index, properties)?;

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
    let gebser_reference = PortableReference::Staged(gebser_index);

    let mut holons_to_add: Vec<PortableReference> = Vec::new();

    holons_to_add.push(briggs_reference);
    holons_to_add.push(gebser_reference);

    test_case.add_related_holons_step(
        book_index,
        RelationshipName(MapString("AUTHORS".to_string())),
        holons_to_add,
    )?;

    test_case.add_commit_step()?;
    test_case.add_ensure_database_count_step(MapInteger(3))?;

    Ok(test_case.clone())
}

/// Fixture for abandoning stages changes..
///
///  H1, H2, H3, etc refer to order of Holons added to staging area.
/// Before the commit process, these Holons are only able to be identified by their index in the staging_area Vec,
/// thefore it is necessary to maintain their order.
/// Each Holon's index can be figured by subtracting 1. Ex H1 is index 0, H2 index 1
///
///
#[fixture]
pub fn simple_abandon_staged_changes_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple AbandonStagedChanges Testcase".to_string(),
        "".to_string(), // //
    );

    test_case.add_ensure_database_count_step(MapInteger(0))?;

    //  ADD STEP:  STAGE:  Book Holon (H1)  //
    let mut book_holon = Holon::new();
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

    //  ADD STEP:  STAGE:  Person 1 Holon (H2)  //
    let mut person_holon_1 = Holon::new();
    person_holon_1
        .with_property_value(
            PropertyName(MapString("first name".to_string())),
            BaseValue::StringValue(MapString("Roger".to_string())),
        )?
        .with_property_value(
            PropertyName(MapString("last name".to_string())),
            BaseValue::StringValue(MapString("Briggs".to_string())),
        )?;

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

    // ADD STEP:  RELATIONSHIP:  Book H1-> Author H2 & H3  //
    test_case.add_related_holons_step(
        0, // source holons
        RelationshipName(MapString("Authored by".to_string())),
        vec![PortableReference::Staged(1), PortableReference::Staged(2)],
    )?;

    // ADD STEP:  ABANDON:  H2
    test_case.add_abandon_staged_changes_step(1)?;

    //  ADD STEP:  WITH PROPERTIES  //
    // Attempt with_properties dance -- expect success on H1 and for H2 BadRequest/NotAccessible response
    //
    // H1
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("".to_string())),
        BaseValue::StringValue(MapString("".to_string())),
    );
    test_case.add_with_properties_step(1, properties)?;
    // H2
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("".to_string())),
        BaseValue::StringValue(MapString("".to_string())),
    );
    let expected_error = test_case.add_with_properties_step(1, properties);
    if expected_error.is_ok() {
        return Err(HolonError::GuardError("with_property_value".to_string()));
    }

    // ADD STEP:  RELATIONSHIP:  Author H2 -> H3  //
    // Attempt add_related_holon dance -- expect BadRequest/NotAccessible response
    let expected_error = test_case.add_related_holons_step(
        0, // source holons
        RelationshipName(MapString("Authored by".to_string())),
        vec![PortableReference::Staged(1), PortableReference::Staged(2)],
    );
    if expected_error.is_ok() {
        return Err(HolonError::GuardError("add_related_holon".to_string()));
    }

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
    test_case.add_abandon_staged_changes_step(3)?;

    // ADD STEP:  ABANDON:  H5
    test_case.add_abandon_staged_changes_step(4)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP:  ENSURE DATABASE COUNT
    test_case.add_ensure_database_count_step(MapInteger(2))?;

    // ADD STEP:  MATCH SAVED CONTENT
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
