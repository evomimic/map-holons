use holons::helpers::define_local_target;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::*;
use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use holons::relationship::RelationshipTarget::*;
use shared_types_holon::BaseType::*;

use shared_types_holon::holon_node::{BaseValue, BaseType};


/// This file creates a HolonDescriptor Holon and its Associated Relationships
/// We can greatly reduce the code-bulk if we re-factored this as an import function that takes
/// a JSON input stream with type definitions expressed as JSON objects.

// This function defines the TypeDescriptor for a HolonDescriptor (but not the HolonDescriptor itself
pub fn define_holon_type_descriptor() -> Holon {

    // ----------------  DEFINE THE
    // META HOLON DESCRIPTOR -------------------------------
    let mut type_descriptor = Holon::new();


    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the characteristics of Holons".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));

    // Add the TypeDescriptor to the Schema
    // type_descriptor.add_relationship("SCHEMA".to_string(), schema.clone());

    type_descriptor
}
// Defines a HolonDescriptor (w/o any relationships)
pub fn define_holon_descriptor() -> Holon {

      let mut holon_descriptor = Holon::new();

    holon_descriptor
}
pub fn define_collection_type_descriptor() -> Holon {

    // ----------------  DEFINE THE
    // META HOLON DESCRIPTOR -------------------------------
    let mut type_descriptor = Holon::new();


    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("CollectionDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the characteristics of Holon Collections".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Collection".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Collection".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in".to_string(), BaseValue::BooleanValue(true));

    type_descriptor
}
// Defines the CollectionDescriptor details, defines the maximum size of any MAP Holon Collection
pub fn define_collection_descriptor() -> Holon {

    let mut holon_descriptor = Holon::new();
    holon_descriptor.with_property_value("max_items".to_string(), BaseValue::IntegerValue(262144));



    holon_descriptor
}



// pub fn add_schema_relationship -> Holon {
//
//
//     // Define the RelationshipDescriptor for HolonDescriptor-HOLON_SUPERTYPE->TypeDescriptor
//     let type_name = "HolonSupertypeRelationshipDescriptor".to_string();
//     let label = "HolonDescriptor-HOLON_SUPERTYPE->TypeDescriptor".to_string();
//     let label_clone = label.clone();
//     let description = format!("Describes the HolonDescriptor {label_clone} relationship.");
// }
//     let mut supertype_descriptor = Holon::new();
//     supertype_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue(type_name))
//         .with_property_value("description".to_string(), BaseValue::StringValue(description))
//         .with_property_value("label".to_string(), BaseValue::StringValue(label))
//         .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
//         .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));
//
//     // Define a RelationshipTarget for the TypeDescriptor
//     let type_descriptor_target = define_local_target(&supertype_descriptor);
//
//     // Define the holon_descriptor (it has no additional properties beyond its TypeDescriptor)
//     let mut holon_descriptor = Holon::new();
//
//
//
//
//     // // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
//     // let mut descriptor = Holon::new();
//     //
//     // // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
//     // descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("RelationshipDescriptor".to_string()))
//     //     .with_property_value("description".to_string(), BaseValue::StringValue(
//     //         "Describes a relationship between two types of holons".to_string()))
//     //     .with_property_value("label".to_string(), BaseValue::StringValue("Relationship Descriptor".to_string()))
//     //     .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
//     //     .with_property_value("version".to_string(), BaseValue::StringValue("0.0.1 -- Semantic Version really be a String?".to_string()))
//     //     .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false));
//
//
//
//
//
//     // TODO: Add version Relationship to SemanticVersion as HolonReference
//     // TODO: Add schema Relationship to SemanticVersion as HolonReference
//     // TODO: Add Relationship to HolonConstraint
//
//
//     holon_descriptor
//
// }
// pub fn define_holon_property_set() -> Holon {
//
//     // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
//     let mut property_set = Holon::new();
//
//     // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
//     property_set.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonConstraint".to_string()))
//         .with_property_value("description".to_string(), BaseValue::StringValue(
//             "Defines the its properties, constraints, relationships, and dances".to_string()))
//         .with_property_value("label".to_string(), BaseValue::StringValue("Holon Descriptor".to_string()))
//         .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
//         .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));
//
//
//
//     property_set
//
// }
//