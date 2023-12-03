// Bootstrap EnumDescriptor
/// This file creates an EnumDescriptor Holon and its Associated EnumVariant Holon


use holons::holon_types::{Holon};
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{PropertyValue,BaseType,SemanticVersion};


pub fn define_enum_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), PropertyValue::StringValue("EnumDescriptor".to_string()))
        .with_property_value("description".to_string(), PropertyValue::StringValue("Describes a MAP Enum element".to_string()))
        .with_property_value("label".to_string(), PropertyValue::StringValue("Enum Descriptor".to_string()))
        .with_property_value("base_type".to_string(), PropertyValue::StringValue("Enum -- Should this really be a String?".to_string()))
        .with_property_value("version".to_string(), PropertyValue::StringValue("0.0.1 -- Semantic Version really be a String?".to_string()))
        .with_property_value("is_dependent".to_string(), PropertyValue::BooleanValue(true));

    // TODO: Add Relationhip to EnumVariant

    descriptor

}

pub fn define_enum_variant() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------------
    descriptor.with_property_value("type_name".to_string(), PropertyValue::StringValue("EnumVariantDescriptor".to_string()))
        .with_property_value("description".to_string(), PropertyValue::StringValue("Describes a single variant of an owning enum".to_string()))
        .with_property_value("label".to_string(), PropertyValue::StringValue("Enum Variant Descriptor".to_string()))
        .with_property_value("base_type".to_string(), PropertyValue::StringValue("Holon -- this should really be an Enum".to_string()))
        .with_property_value("version".to_string(), PropertyValue::StringValue("0.0.1 -- this should really be an Enum".to_string()))
        .with_property_value("is_dependent".to_string(), PropertyValue::BooleanValue(true));

    descriptor
}

