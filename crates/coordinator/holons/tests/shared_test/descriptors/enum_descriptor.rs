// Bootstrap EnumDescriptor
/// This file creates an EnumDescriptor Holon and its Associated EnumVariant Holon


use holons::holon_types::{Holon};


use shared_types_holon::holon_node::{BaseValue};
use shared_types_holon::value_types::BaseType;


pub fn define_enum_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("EnumDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a MAP Enum value whose variants are simple strings".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Enum Value Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::EnumValue("Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    // TODO: Add Relationhip to EnumVariantDescriptor

    descriptor

}
/// Enum
pub fn define_enum_variant_descriptor(base_type: BaseType) -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  ADD TYPE_HEADER PROPERTIES -----------------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("EnumVariantDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue("Describes a single variant of an owning enum".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Enum Variant Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::EnumValue(base_type.to_string()))
        .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    descriptor
}

