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
use dances::dance_response::ResponseStatusCode;
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
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;

use shared_types_holon::{
    MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn load_core_schema_test_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Load the MAP Core (L0) Schema Testcase".to_string(),
        "Bulk of work is on the guest-side load_core_schema function".to_string(),
    );

    //let mut expected_holons = Vec::new();

    test_case.add_ensure_database_count_step(MapInteger(1))?;

    test_case.add_load_core_schema()?;

    test_case.add_database_print_step()?;

    //expected_holons.push(book_holon.clone());

    //test_case.add_ensure_database_count_step(MapInteger(1))?;

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
