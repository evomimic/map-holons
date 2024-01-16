// This file creates a TypeDescriptor for the different MAP Value Types


use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;

use shared_types_holon::value_types::BaseType;
use shared_types_holon::holon_node::{BaseValue};


// Is a generic TypeDescriptor function needed?

pub fn define_type_descriptor() -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();


    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("TypeDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "A meta-descriptor that defines the properties, relationships and dances shared by all MAP descriptors (including itself).".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Type Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_value_descriptor".to_string(), BaseValue::BooleanValue(false));



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
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("SemanticVersion".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Supports a structured approach to tracking changes to a chain of TypeDescriptor versions.".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Semantic Version".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    descriptor

}


pub fn define_type_descriptor_to_semantic_version(schema_target: &RelationshipTarget) -> Holon {

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();


    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("TypeDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the TypeDescriptor supertype".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Type Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    /* TODO: Define SemanticVersionDescriptor,
        define TypeDescriptor-VERSION->SemanticVersion RelationshipDescriptor
        ask SemanticVersionDescriptor to define a SemanticVersion
        then add a version Relationship from TypeDescriptor to SemanticVersion
    */
    descriptor

}

