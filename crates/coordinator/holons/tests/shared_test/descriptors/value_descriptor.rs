// This file creates the descriptors for the built-in MAP Value Types


use holons::holon_types::{Holon};
use shared_types_holon::value_types::BaseType;

use shared_types_holon::holon_node::{BaseValue};
// I don't think ValueDescriptor is needed as an intermediate supertype
// between specific ValueDescriptors and TypeDescriptor --
// pub fn define_value_type_descriptor() -> Holon {
//
//     let mut descriptor = Holon::new();
//
//     descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("ValueDescriptor".to_string()))
//         .with_property_value("description".to_string(), BaseValue::StringValue(
//             "Describes the supertype of all MAP ValueDescriptors".to_string()))
//         .with_property_value("label".to_string(), BaseValue::StringValue("Value Descriptor".to_string()))
//         .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
//         .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
//         .with_property_value("is_built_in_type".to_string(), BaseValue::BooleanValue(true));
//
//     descriptor
//
// }
//
// pub fn define_value_descriptor() -> Holon {
//     let mut descriptor = Holon::new();
//
//     descriptor
// }
pub fn define_string_type_descriptor() -> Holon {

    let mut type_descriptor = Holon::new();
    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("String Value Descriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the built-in String Value type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("String Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in_type".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}
pub fn define_string_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max lengths on this builtin descriptor specify the min and max lengths of ANY MAP String


   descriptor.with_property_value("min_length".to_string(), BaseValue::IntegerValue(0))
       .with_property_value("max_length".to_string(), BaseValue::IntegerValue(8192));

   descriptor

}

pub fn define_integer_type_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut type_descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("IntegerDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a builtin Integer Value Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Integer Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in_type".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}
pub fn define_integer_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max values on this builtin descriptor specify the min and max values of ANY MAP Integer


    descriptor.with_property_value("min_value".to_string(), BaseValue::IntegerValue(-9223372036854775808))
        .with_property_value("max_value".to_string(), BaseValue::IntegerValue(9223372036854775807));

    descriptor

}


pub fn define_boolean_type_descriptor() -> Holon {

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
pub fn define_boolean_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();


    descriptor.with_property_value("is_fuzzy".to_string(), BaseValue::BooleanValue(false));

    descriptor

}
pub fn define_enum_type_descriptor() -> Holon {

    let mut type_descriptor = Holon::new();

    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("EnumDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a MAP Enum Value Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Enum Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}
pub fn define_enum_descriptor() -> Holon {

    let mut enum_descriptor = Holon::new();

    enum_descriptor

}

pub fn define_enum_variant_type_descriptor() -> Holon {

    let mut type_descriptor = Holon::new();

    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("EnumVariantDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a a specific variant of an Enum Value Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Enum Variant Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::String".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}
pub fn define_enum_variant_descriptor() -> Holon {

    let mut enum_descriptor = Holon::new();

    enum_descriptor

}

pub fn define_value_array_type_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut type_descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("ValueArrayDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the builtin Value Array Type".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("ValueArray Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in_type".to_string(), BaseValue::BooleanValue(true));

    type_descriptor

}
pub fn define_value_array_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max values on this builtin descriptor specify the min and max values of ANY MAP Integer


    descriptor.with_property_value("max_items".to_string(), BaseValue::IntegerValue(1024));

    descriptor

}

