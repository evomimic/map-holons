use holochain::prelude::VecOrSingle::Vec;
/// This file defines the a RelationshipDescriptor
///
use holons::holon_types::Holon;
use holons::relationship::HolonCollection;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{
    BaseType, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

pub fn define_relationship_type_descriptor() -> Holon {
    let mut type_descriptor = Holon::new();

    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("RelationshipDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes a relationship between two holons".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Relationship Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("BaseType::Holon".to_string())),
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

pub fn define_relationship_descriptor() -> Holon {
    let mut relationship_descriptor = Holon::new();

    relationship_descriptor
        .with_property_value(
            PropertyName(MapString("min_target_cardinality".to_string())),
            BaseValue::IntegerValue(MapInteger(0)),
        )
        .with_property_value(
            PropertyName(MapString("max_target_cardinality".to_string())),
            BaseValue::IntegerValue(MapInteger(262144)),
        )
        .with_property_value(
            PropertyName(MapString("affinity".to_string())),
            BaseValue::IntegerValue(MapInteger(100)),
        );

    relationship_descriptor
}
