// This file creates an Holon Space Holon

use holons::holon_types::Holon;
use shared_types_holon::value_types::{BaseType, BaseValue};

pub fn new_holon_space() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut holon_space = Holon::new();

    holon_space.with_property_value("name".to_string(), BaseValue::StringValue("Local Holon Space".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "The top-level local container for local holons, relatioships, and proxies to/from external holon spaces".to_string()));

    // TODO: Add holons relationship to contained holons and descriptor relationship to the HolonSpaceDescriptor

    holon_space
}
pub fn define_holon_space_descriptor() -> Holon {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonSpace".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes a MAP Holon Space, including its properties, constraints, relationships, and dances".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Space".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));

    // TODO: Add Relationhips

    descriptor
}
