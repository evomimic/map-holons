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
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::context::HolonsContext;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

use crate::shared_test::test_data_types::DancesTestCase;

// use hdk::prelude::*;

// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };

use holons::holon_error::HolonError;
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

    test_case.add_ensure_database_count_step(MapInteger(0))?;

    let mut book_holon = Holon::new();
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    );
    test_case.add_stage_holon_step(book_holon)?;

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string()))
    );

    test_case.add_with_properties_step(MapInteger(0), properties)?;

    let mut person_holon = Holon::new();

    test_case.add_stage_holon_step(person_holon)?;

    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(MapInteger(1), properties)?;

    test_case.add_commit_step()?;
    test_case.add_ensure_database_count_step(MapInteger(2))?;

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

    Ok(test_case.clone())
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_create_dummy_data() {
//         let data = create_dummy_data(()).unwrap();

//         println!("{:#?}", data);
//     }
// }
