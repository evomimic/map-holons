use holochain::prelude::VecOrSingle::Vec;
/// This file defines the a RelationshipDescriptor
///


use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};


pub fn define_relationship_type_descriptor() -> Holon {

    let mut type_descriptor = Holon::new();

    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("RelationshipDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a relationship between two holons".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Relationship Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}

pub fn define_relationship_descriptor() -> Holon {

    let mut relationship_descriptor = Holon::new();

    relationship_descriptor
        .with_property_value("min_target_cardinality".to_string(), BaseValue::IntegerValue(0))
        .with_property_value("max_target_cardinality".to_string(), BaseValue::IntegerValue(262144))
        .with_property_value("affinity".to_string(), BaseValue::IntegerValue(100));


    relationship_descriptor

}


