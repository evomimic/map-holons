/// This file creates an Schema Holon


use holons::holon_types::{Holon};
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};


pub fn define_schema() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut schema = Holon::new();

    schema.with_property_value("name".to_string(), BaseValue::StringValue("MAP L0 Core".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "The foundational MAP type descriptors".to_string()));


    // TODO: Add Relationship to TypeDescriptor holon

    schema

}
