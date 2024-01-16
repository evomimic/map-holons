
/// This file creates a Schema Holon and all of its child descriptors comprising the L0 layer
/// of the MAP Ontology as well as the relationships between those descriptors


use holons::holon_types::{Holon};
use shared_types_holon::value_types::BaseType;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::Local;
use holons::relationship::RelationshipTarget;
use holons::helpers::*;
use shared_types_holon::holon_node::{BaseValue};
use crate::shared_test::descriptors::descriptor_types::TYPE_DESCRIPTION_TEMPLATE;
use crate::shared_test::descriptors::holon_descriptor::*;
use crate::shared_test::descriptors::property_descriptor::*;
use crate::shared_test::descriptors::type_descriptor::*;
use crate::shared_test::descriptors::relationship_descriptor::*;
use crate::shared_test::descriptors::value_descriptor::*;

pub fn define_schema() -> Holon {

    // 1) Define the, initially undescribed, Schema Holon
    // 2) Define all the Descriptors (w/o relationships)
    // 3) Add the relationships between all of the Descriptors

    let mut schema = Holon::new();

    schema.with_property_value("name".to_string(), BaseValue::StringValue("MAP L0 Core".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string()));

    // Define a RelationshipTarget for the Schema Holon

    let schema_target = define_local_target(&schema);

    //
    // Now create all the Core L0 Descriptors
    //


    let schema_descriptor = define_schema_descriptor(&schema_target);
    //let schema_descriptor = define_schema_descriptor();

    let holon_type_descriptor = define_holon_type_descriptor();
    let holon_descriptor = define_holon_descriptor();

    let collection_type_descriptor = define_collection_type_descriptor();
    let collection_descriptor = define_collection_descriptor();

    let relationship_type_descriptor = define_relationship_type_descriptor();
    let relationship_descriptor = define_relationship_descriptor();

    // Define Builtin Value Type Descriptors
    let string_type_descriptor = define_string_type_descriptor();
    let string_descriptor = define_string_descriptor();

    let integer_type_descriptor = define_integer_type_descriptor();
    let integer_descriptor = define_integer_descriptor();

    let boolean_type_descriptor = define_boolean_type_descriptor();
    let boolean_descriptor = define_boolean_descriptor();

    let enum_type_descriptor =define_enum_type_descriptor();
    let enum_type_descriptor =define_enum_descriptor();

    let enum_variant_type_descriptor =define_enum_variant_type_descriptor();
    let enum_variant_descriptor =define_enum_variant_descriptor();


    let property_type_descriptor = define_property_type_descriptor();
    let property_descriptor = define_property_descriptor();

    let value_array_type_descriptor= define_value_array_type_descriptor();
    let value_array_descriptor= define_value_array_descriptor();

    //
    // Define the relationship descriptors that relate the above descriptors
    //






    schema

}
// pub fn define_schema_type_descriptor(schema_target: &RelationshipTarget)-> Holon {
// // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
//
// }

pub fn define_schema_descriptor(schema_target: &RelationshipTarget) -> Holon {
    let mut schema_type_descriptor = Holon::new();

    schema_type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("SchemaDescriptor".to_string()))
    .with_property_value("description".to_string(), BaseValue::StringValue("Descriptor for Schema".to_string()))
    .with_property_value("label".to_string(), BaseValue::StringValue("Schema Descriptor".to_string()))
    .with_property_value("base_type".to_string(), BaseValue::EnumValue("BaseType::Holon".to_string()))
    .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    schema_type_descriptor.add_related_holon("TypeDescriptor-INSTANCES->Schema".to_string(), Some(schema_target.clone()));

    let type_descriptor_target = define_local_target(&schema_type_descriptor);

    /* TODO: Define SemanticVersionDescriptor,
    define TypeDescriptor-VERSION->SemanticVersion RelationshipDescriptor
    ask SemanticVersionDescriptor to define a SemanticVersion
    then add a version Relationship from TypeDescriptor to SemanticVersion
    */
    let mut schema_descriptor = Holon::new();
    schema_descriptor.

    schema_descriptor

}
/// This function defines the Schema -DESCRIPTORS-> TypeDescriptor relationship descriptor
/// and adds it to the schema_target
pub fn define_schema_relationship_descriptor(schema_target: &RelationshipTarget)-> Holon {
// ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut schema_relationship_descriptor = Holon::new();

    schema_relationship_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("SchemaDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the TypeDescriptor supertype".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));

    /* TODO: Define SemanticVersionDescriptor,
    define TypeDescriptor-VERSION->SemanticVersion RelationshipDescriptor
    ask SemanticVersionDescriptor to define a SemanticVersion
    then add a version Relationship from TypeDescriptor to SemanticVersion
*/
}





