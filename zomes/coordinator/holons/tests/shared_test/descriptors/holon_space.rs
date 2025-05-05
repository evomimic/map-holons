// This file creates an Holon Space Holon

use holons::holon_types::Holon;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{
    TypeKind, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

pub fn new_holon_space() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut holon_space = Holon::new();

    holon_space.with_property_value(PropertyName(MapString("name".to_string())), BaseValue::StringValue(MapString("Local Holon Space".to_string())))
        .with_property_value(PropertyName(MapString("description".to_string())), BaseValue::StringValue(
            MapString("The top-level local container for local holons, relatioships, and proxies to/from external holon spaces".to_string())));

    // TODO: Add holons relationship to contained holons and descriptor relationship to the HolonSpaceDescriptor

    holon_space
}
pub fn define_holon_space_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value(PropertyName(MapString("type_name".to_string())), BaseValue::StringValue(MapString("HolonSpace".to_string())))
        .with_property_value(PropertyName(MapString("description".to_string())), BaseValue::StringValue(
            MapString("Describes a MAP Holon Space, including its properties, constraints, relationships, and dances".to_string())))
        .with_property_value(PropertyName(MapString("label".to_string())), BaseValue::StringValue(MapString("Holon Space".to_string())))
        .with_property_value(PropertyName(MapString("base_type".to_string())), BaseValue::StringValue(MapString("TypeKind::Holon".to_string())))
        .with_property_value(PropertyName(MapString("is_dependent".to_string())), BaseValue::BooleanValue(MapBoolean(false)));

    // TODO: Add Relationhips

    descriptor
}
