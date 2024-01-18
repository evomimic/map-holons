// This file defines the TypeDescriptor struct and the dance functions it supports

use holons::helpers::define_local_target;
use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;

use crate::semantic_version::define_semantic_version;
use shared_types_holon::value_types::{
    BaseType, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

// This is a helper function for defining new TypeDescriptor holons
// It populates the TypeDescriptor's property_map from the supplied parameters
// and adds the following relationships to the TypeDescriptors relationship_map:
//     TypeDescriptor-COMPONENT_OF>Schema (for supplied schema_target)
//     TypeDescriptor-VERSION->SemanticVersion (for default version)
//     TypeDescriptor-HAS_PROPERTIES->PropertyDescriptor (empty)
//     TypeDescriptor-HAS_OUTBOUND-> RelationshipDescriptor (empty),

pub fn define_type_descriptor(
    schema: &RelationshipTarget,
    type_name: String,
    base_type: BaseType,
    description: String,
    label: String, // Human readable name for this type
    is_dependent: bool,
    is_value_descriptor: bool,
) -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            MapString("type_name".to_string()),
            BaseValue::StringValue(MapString(type_name)),
        )
        .with_property_value(
            MapString("description".to_string()),
            BaseValue::StringValue(MapString(description)),
        )
        .with_property_value(
            MapString("label".to_string()),
            BaseValue::StringValue(MapString(label)),
        )
        .with_property_value(
            MapString("base_type".to_string()),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )
        .with_property_value(
            MapString("is_dependent".to_string()),
            BaseValue::BooleanValue(MapBoolean(is_dependent)),
        )
        .with_property_value(
            MapString("is_value_descriptor".to_string()),
            BaseValue::BooleanValue(MapBoolean(is_value_descriptor)),
        );

    // Define a default semantic_version
    let version = define_semantic_version(0, 0, 1);

    // Add the outbound relationships shared by all TypeDescriptors
    let version_target = define_local_target(&version);

    descriptor
        .add_related_holon(MapString("COMPONENT_OF".to_string()), Some(schema.clone()))
        .add_related_holon(MapString("VERSION".to_string()), Some(version_target));

    descriptor
}
