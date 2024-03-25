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
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use std::collections::btree_map::BTreeMap;

// use hdk::prelude::*;

use crate::shared_test::test_data_types::HolonCreatesTestCase;
// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };

use holons::holon_error::HolonError;
use shared_types_holon::{MapBoolean, MapString, PropertyName};

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn undescribed_holons_fixture() -> Result<HolonCreatesTestCase, HolonError> {
    let mut test_data_set: Vec<Holon> = Vec::new();

    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("TypeDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("descriptor_name".to_string())),
            BaseValue::StringValue(MapString(
                "This holon does not have a descriptor".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "this is a simple TypeDescriptor holon".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Type Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        );
    test_data_set.push(descriptor);

    Ok(HolonCreatesTestCase {
        creates: test_data_set,
    })
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
