use holochain::prelude::VecOrSingle::Vec;
/// This file defines the a RelationshipDescriptor
///


use holons::holon_types::{Holon};
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};


pub fn define_relationship_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
        let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("RelationshipDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a relationship between two types of holons".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Relationship Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1 -- Semantic Version really be a String?".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));



    // TODO: Add Relationhip to EnumVariantDescriptor

    descriptor

}
pub fn define_holon_property_set() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut property_set = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    property_set.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonConstraint".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Defines the its properties, constraints, relationships, and dances".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    // TODO: Add Relationship to HolonConstraint

    property_set

}