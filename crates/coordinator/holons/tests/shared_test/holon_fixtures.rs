// Test Dataset Creator
//
// This file is used to create data used to test the following capabilities:
// - get all holons_integrity
// - build new holon
// - create holon
// - get holon
// - delete holon
//
//
// The logic for CUD tests is identical, what varies is the test data.
// BUT... if the test data set has all different variations in it, we may only need 1 test data set

#![allow(dead_code)]


use core::panic;
use std::collections::btree_map::BTreeMap;
use rstest::*;
use holons::helpers::*;
use holons::holon::{Holon};
use holons::holon_api::*;
use shared_types_holon::holon_node::PropertyValue;



// use hdk::prelude::*;

use crate::shared_test::test_data_types::{HolonCreatesTestCase};
// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };
use holons::holon_errors::HolonError;

/// This function creates a rich test dataset by creating a vector of Holons of various
/// kinds -- from simple to complex
#[fixture]
pub fn new_holons_fixture() -> Result<HolonCreatesTestCase, HolonError> {
    let mut test_data_set: Vec<Holon> = Vec::new();

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut holon = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD SOME PROPERTIES -----------------------
    // TODO: Should we use external holon_api?

    holon.add_property_value("type_name".to_string(), PropertyValue::StringValue("ValueDescriptor".to_string()))
        .add_property_value("description".to_string(), PropertyValue::StringValue("Provides description of a ValueType".to_string()))
        .add_property_value("label".to_string(), PropertyValue::StringValue("Value Type Descriptor".to_string()))
        .add_property_value("base_type".to_string(), PropertyValue::StringValue("Holon -- this should really be an Enum".to_string()))
        .add_property_value("version".to_string(), PropertyValue::StringValue("0.0.1 -- this should really be an Enum".to_string()))
        .add_property_value("is_dependent".to_string(), PropertyValue::BooleanValue(true));

    test_data_set.push(holon);

   Ok(HolonCreatesTestCase {creates: test_data_set})
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
