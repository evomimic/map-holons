// Test Dataset Creator
//
// This file is used to create data used to test the following capabilities:
// - get all type_descriptors
// - build new type descriptor
// - commit the new type descriptor
// - get the new type descriptor
// - delete holon
//
//
// The logic for CUD tests is identical, what varies is the test data.
// BUT... if the test data set has all different variations in it, we may only need 1 test data set

#![allow(dead_code)]

use core::panic;
use descriptors::descriptor_types::{
    Schema, META_PROPERTY_DESCRIPTOR, META_RELATIONSHIP_DESCRIPTOR, META_TYPE_DESCRIPTOR,
};
use descriptors::holon_descriptor::define_holon_descriptor;
use descriptors::type_descriptor::define_type_descriptor;
use holons::cache_manager::HolonCacheManager;
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use rstest::*;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapString};
use std::collections::btree_map::BTreeMap;

// use hdk::prelude::*;

use crate::shared_test::test_data_types::{DescriptorTestCase, DescriptorTestStep};
// use crate::shared_test::fixture_helpers::{derive_label, derive_type_description, derive_type_name};
// use crate::shared_test::property_descriptor_data_creators::{
//     create_example_property_descriptors, create_example_updates_for_property_descriptors,
// };
use holons::holon_errors::HolonError;

/// This function creates returns a TestCase containing a sequence of Descriptor Create, Update and Delete ops
///
#[fixture]
pub fn descriptors_fixture() -> Result<DescriptorTestCase, HolonError> {
    let mut context = HolonsContext {
        commit_manager: CommitManager::new().into(),
        cache_manager: HolonCacheManager::new().into(),
    };

    let mut schema = Schema::new(
        "MAP L0 Core Schema".to_string(),
        "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string(),
    );

    let rc_schema = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(schema.0); // Borrow_mut() allows mutation

    let mut steps: Vec<DescriptorTestStep> = Vec::new();

    // let schema_reference = define_local_target(&schema.into_holon());
    let type_descriptor = define_type_descriptor(&context,
                                                 rc_schema.clone(),
                                                 MapString(META_TYPE_DESCRIPTOR.to_string()),
                                                 MapString("TypeDescriptor".to_string()),
                                                 BaseType::Holon,
                                                 MapString("A meta-descriptor that defines the properties and relationships shared by all MAP descriptors (including itself).".to_string()),
                                                 MapString("Meta Type Descriptor".to_string()),
                                                 MapBoolean(false),
                                                 MapBoolean(false),
                                                 None,
                                                 None);

    // Add to Schema-COMPONENTS->TypeDescriptor relationships?
    steps.push(DescriptorTestStep::Create(type_descriptor.0.clone()));

    let meta_holon_descriptor = define_holon_descriptor(&context,
                                                        rc_schema.clone(),
                                                        MapString("HolonDescriptor".to_string()),
                                                        MapString("A meta-descriptor that defines the properties and relationships shared by all MAP HolonDescriptors".to_string()),
                                                        MapString("Meta Holon Descriptor".to_string()),
                                                        None,
                                                        //Some(HolonReference::Local((LocalHolonReference::from_holon((type_descriptor.as_holon()))))),
                                                        None);

    steps.push(DescriptorTestStep::Create(meta_holon_descriptor.0.clone()));
    let meta_relationship_descriptor = define_type_descriptor(&context,
                                                              rc_schema.clone(),
                                                              MapString(META_RELATIONSHIP_DESCRIPTOR.to_string()),
                                                              MapString("RelationshipDescriptor".to_string()),
                                                              BaseType::Holon,
                                                              MapString("A meta-descriptor that defines the properties and relationships shared by all MAP RelationshipDescriptors".to_string()),
                                                              MapString("Meta Relationship Descriptor".to_string()),
                                                              MapBoolean(false),
                                                              MapBoolean(false),
                                                              None,
                                                              None);
    steps.push(DescriptorTestStep::Create(
        meta_relationship_descriptor.0.clone(),
    ));

    let meta_property_descriptor = define_type_descriptor(&context,
                                                          rc_schema.clone(),
                                                          MapString(META_PROPERTY_DESCRIPTOR.to_string()),
                                                          MapString("PropertyDescriptor".to_string()),
                                                          BaseType::Holon,
                                                          MapString("A meta-descriptor that defines the properties and relationships shared by all MAP PropertyDescriptors".to_string()),
                                                          MapString("Property Meta Descriptor".to_string()),
                                                          MapBoolean(false),
                                                          MapBoolean(false),
                                                          None,
                                                          None);

    steps.push(DescriptorTestStep::Create(
        meta_property_descriptor.0.clone(),
    ));

    let test_case = DescriptorTestCase { steps };
    Ok(test_case)
}
