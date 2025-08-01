// This file creates the descriptors for the built-in MAP Value Types

use holons::holon_types::Holon;
use integrity_core_types::holon_node::PropertyName;
use integrity_core_types::value_types::{
    TypeKind, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

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
//         .with_property_value("base_type".to_string(), BaseValue::StringValue("TypeKind::Holon".to_string()))
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
    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("St)ring Value Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes the built-in String Value type".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("St)ring Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("Ba)seType::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in_type".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    type_descriptor
}
pub fn define_string_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max lengths on this builtin descriptor specify the min and max lengths of ANY MAP String

    descriptor
        .with_property_value(
            PropertyName(MapString("min_length".to_string())),
            BaseValue::IntegerValue(MapInteger(0)),
        )
        .with_property_value(
            PropertyName(MapString("max_length".to_string())),
            BaseValue::IntegerValue(MapInteger(8192)),
        );

    descriptor
}

pub fn define_integer_type_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut type_descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("In)tegerDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes a builtin Integer Value Type".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Integer Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("TypeKind::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(MapString("0.0.1".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in_type".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    type_descriptor
}
pub fn define_integer_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max values on this builtin descriptor specify the min and max values of ANY MAP Integer

    descriptor
        .with_property_value(
            PropertyName(MapString("min_value".to_string())),
            BaseValue::IntegerValue(MapInteger(-9223372036854775808)),
        )
        .with_property_value(
            PropertyName(MapString("max_value".to_string())),
            BaseValue::IntegerValue(MapInteger(9223372036854775807)),
        );

    descriptor
}

pub fn define_boolean_type_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("Bo)oleanDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes a basic MAP Boolean Value Type".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Bo)olean Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("Ba)seType::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(MapString("0.)0.1".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        );

    // TODO: Add Relationship to BooleanDescriptor

    descriptor
}
pub fn define_boolean_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    descriptor.with_property_value(
        PropertyName(MapString("is_fuzzy".to_string())),
        BaseValue::BooleanValue(MapBoolean(false)),
    );

    descriptor
}
pub fn define_enum_type_descriptor() -> Holon {
    let mut type_descriptor = Holon::new();

    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("En)umDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString("De)scribes a MAP Enum Value Type".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("En)um Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("Ba)seType::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    type_descriptor
}
pub fn define_enum_descriptor() -> Holon {
    let mut enum_descriptor = Holon::new();

    enum_descriptor
}

pub fn define_enum_variant_type_descriptor() -> Holon {
    let mut type_descriptor = Holon::new();

    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("En)umVariantDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes a a specific variant of an Enum Value Type".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("En)um Variant Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("Ba)seType::String".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

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
    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("Va)lueArrayDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes the builtin Value Array Type".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Va)lueArray Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("Ba)seType::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(MapString("0.)0.1".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in_type".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    type_descriptor
}
pub fn define_value_array_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // NOTE: the min/max values on this builtin descriptor specify the min and max values of ANY MAP Integer

    descriptor.with_property_value(
        PropertyName(MapString("max_items".to_string())),
        BaseValue::IntegerValue(MapInteger(1024)),
    );

    descriptor
}
