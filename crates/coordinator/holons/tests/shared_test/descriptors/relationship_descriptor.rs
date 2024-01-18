use holochain::prelude::VecOrSingle::Vec;
/// This file defines the a RelationshipDescriptor
///
use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::{BaseType, BaseValue, MapString, MapBoolean, MapInteger, MapEnumValue};

pub fn define_relationship_type_descriptor() -> Holon {
    let mut type_descriptor = Holon::new();

    type_descriptor
        .with_property_value(
            MapString("type_name".to_string()),
            BaseValue::StringValue(MapString("RelationshipDescriptor".to_string())),
        )
        .with_property_value(
            MapString("description".to_string()),
            BaseValue::StringValue(MapString("Describes a relationship between two holons".to_string())),
        )
        .with_property_value(
            MapString("label".to_string()),
            BaseValue::StringValue(MapString("Relationship Descriptor".to_string())),
        )
        .with_property_value(
            MapString("base_type".to_string()),
            BaseValue::StringValue(MapString("BaseType::Holon".to_string())),
        )
        .with_property_value(MapString("is_dependent".to_string()), BaseValue::BooleanValue(MapBoolean(false)))
        .with_property_value(MapString("is_built_in".to_string()), BaseValue::BooleanValue(MapBoolean(true)));

    type_descriptor
}

pub fn define_relationship_descriptor() -> Holon {
    let mut relationship_descriptor = Holon::new();

    relationship_descriptor
        .with_property_value(
            MapString("min_target_cardinality".to_string()),
            BaseValue::IntegerValue(MapInteger(0)),
        )
        .with_property_value(
            MapString("max_target_cardinality".to_string()),
            BaseValue::IntegerValue(MapInteger(262144)),
        )
        .with_property_value(MapString("affinity".to_string()), BaseValue::IntegerValue(MapInteger(100)));

    relationship_descriptor
}
