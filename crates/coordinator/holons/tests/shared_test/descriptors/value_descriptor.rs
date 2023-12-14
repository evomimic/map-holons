// This file creates a TypeDescriptors for the different MAP Value Types


use holons::holon_types::{Holon};
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};


pub fn define_string_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("StringDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a MAP String Value".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("String Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));

    // TODO: Add Relationship to StringDescriptor

    descriptor

}

pub fn define_integer_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("IntegerDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a MAP Integer Value Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Integer Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));


    // TODO: Add Relationships to IntegerDescriptor for
    // PropertyDescriptorMpa

    descriptor

}

pub fn define_boolean_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("BooleanDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a basic MAP Boolean Value Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Boolean Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));

    // TODO: Add Relationship to BooleanDescriptor

    descriptor

}