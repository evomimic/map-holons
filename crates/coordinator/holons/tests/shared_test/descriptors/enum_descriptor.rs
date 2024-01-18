// Bootstrap EnumDescriptor
/// This file creates an EnumDescriptor Holon and its Associated EnumVariant Holon
use holons::holon_types::Holon;

use shared_types_holon::value_types::{
    BaseType, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

pub fn define_enum_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            MapString("type_name".to_string()),
            BaseValue::StringValue(MapString("EnumDescriptor".to_string())),
        )
        .with_property_value(
            MapString("description".to_string()),
            BaseValue::StringValue(MapString(
                "Describes a MAP Enum value whose variants are simple strings".to_string(),
            )),
        )
        .with_property_value(
            MapString("label".to_string()),
            BaseValue::StringValue(MapString("Enum Value Descriptor".to_string())),
        )
        .with_property_value(
            MapString("base_type".to_string()),
            BaseValue::EnumValue(MapEnumValue(MapString("Holon".to_string()))),
        )
        .with_property_value(
            MapString("is_dependent".to_string()),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    // TODO: Add Relationhip to EnumVariantDescriptor

    descriptor
}
/// Enum
pub fn define_enum_variant_descriptor(base_type: BaseType) -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  ADD TYPE_HEADER PROPERTIES -----------------------
    descriptor
        .with_property_value(
            MapString("type_name".to_string()),
            BaseValue::StringValue(MapString("EnumVariantDescriptor".to_string())),
        )
        .with_property_value(
            MapString("description".to_string()),
            BaseValue::StringValue(MapString(
                "Describes a single variant of an owning enum".to_string(),
            )),
        )
        .with_property_value(
            MapString("label".to_string()),
            BaseValue::StringValue(MapString("Enum Variant Descriptor".to_string())),
        )
        .with_property_value(
            MapString("base_type".to_string()),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )
        .with_property_value(
            MapString("version".to_string()),
            BaseValue::StringValue(MapString("0.0.1".to_string())),
        )
        .with_property_value(
            MapString("is_dependent".to_string()),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    descriptor
}
