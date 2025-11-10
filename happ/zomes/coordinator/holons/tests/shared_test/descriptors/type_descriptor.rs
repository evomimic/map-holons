// This file creates a TypeDescriptor for the different MAP Value Types

use holons::holon_types::Holon;
use holons::relationship::HolonCollection;
use integrity_core_types::holon_node::PropertyName;
use integrity_core_types::value_types::{
    TypeKind, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

// Is a generic TypeDescriptor function needed?

pub fn define_type_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value(PropertyName(MapString("type_name".to_string())), BaseValue::StringValue(MapString("TypeDescriptor".to_string())))
        .with_property_value(PropertyName(MapString("description".to_string())), BaseValue::StringValue(MapString(("A meta-descriptor that defines the properties, relationships and dances shared by all MAP descriptors (including itself).".to_string()))))
        .with_property_value(PropertyName(MapString("label".to_string())), BaseValue::StringValue(MapString("Type Descriptor".to_string())))
        .with_property_value(PropertyName(MapString("base_type".to_string())), BaseValue::StringValue(MapString("TypeKind::Holon".to_string())))
        .with_property_value(PropertyName(MapString("is_dependent".to_string())), BaseValue::BooleanValue(MapBoolean(false)))
        .with_property_value(PropertyName(MapString("is_value_descriptor".to_string())), BaseValue::BooleanValue(MapBoolean(false)));

    /* TODO: Define SemanticVersionDescriptor,
        define TypeDescriptor-VERSION->SemanticVersion RelationshipDescriptor
        ask SemanticVersionDescriptor to define a SemanticVersion
        then add a version Relationship from TypeDescriptor to SemanticVersion
    */
    descriptor
}
pub fn define_semantic_version_descriptor() -> Holon {
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value(PropertyName(MapString("type_name".to_string())), BaseValue::StringValue(MapString("SemanticVersion".to_string())))
        .with_property_value(PropertyName(MapString("description".to_string())), BaseValue::StringValue(
            MapString("Supports a structured approach to tracking changes to a chain of TypeDescriptor versions.".to_string())))
        .with_property_value(PropertyName(MapString("label".to_string())), BaseValue::StringValue(MapString("Semantic Version".to_string())))
        .with_property_value(PropertyName(MapString("base_type".to_string())), BaseValue::StringValue(MapString("TypeKind::Holon".to_string())))
        .with_property_value(PropertyName(MapString("is_dependent".to_string())), BaseValue::BooleanValue(MapBoolean(true)));

    descriptor
}

pub fn define_type_descriptor_to_semantic_version(schema_target: &HolonCollection) -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("TypeDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes the TypeDescriptor supertype".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Type Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("TypeKind::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    /* TODO: Define SemanticVersionDescriptor,
        define TypeDescriptor-VERSION->SemanticVersion RelationshipDescriptor
        ask SemanticVersionDescriptor to define a SemanticVersion
        then add a version Relationship from TypeDescriptor to SemanticVersion
    */
    descriptor
}
