/// This file creates a HolonDescriptor Holon and its Associated Relationships


use holons::holon_types::{Holon};
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};

// TODO: change the return type to Vec<Holon>
// Create the TypeDescriptor Holon
// Create the HolonDescriptor Holon
// Create the supertype RelationshipDescriptor Holon
// Add the supertype relationship to the HolonDescriptor Holon -- note that this should happen
// on the Coordinator Zome level HolonDescriptor type.
pub fn define_holon_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();


    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("TypeDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the TypeDescriptor supertype".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Type Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));

    // TODO: Add version Relationship to SemanticVersion as HolonReference
    // TODO: Add schema Relationship to SemanticVersion as HolonReference
    // TODO: Add Relationship to HolonConstraint


    descriptor

}
pub fn define_holon_property_set() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut property_set = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    property_set.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonConstraint".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Defines the its properties, constraints, relationships, and dances".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));



    property_set

}
//