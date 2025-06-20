use descriptors::{
    boolean_descriptor::BooleanTypeDefinition, enum_descriptor::EnumTypeDefinition,
    enum_variant_descriptor::EnumVariantTypeDefinition, holon_descriptor::HolonTypeDefinition,
    integer_descriptor::IntegerTypeDefinition, property_descriptor::PropertyTypeDefinition,
    relationship_descriptor::RelationshipTypeDefinition, string_descriptor::StringTypeDefinition,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type_kind")]
pub enum CoreTypeEntry {
    #[serde(rename = "Value(Boolean)")]
    Boolean(BooleanTypeDefinition),

    #[serde(rename = "Relationship")]
    Relationship(RelationshipTypeDefinition),

    #[serde(rename = "Property")]
    Property(PropertyTypeDefinition),

    #[serde(rename = "Value(String)")]
    String(StringTypeDefinition),

    #[serde(rename = "Value(Integer)")]
    Integer(IntegerTypeDefinition),

    #[serde(rename = "Enum")]
    Enum(EnumTypeDefinition),

    #[serde(rename = "EnumVariant")]
    EnumVariant(EnumVariantTypeDefinition),

    #[serde(rename = "Holon")]
    Holon(HolonTypeDefinition),
}

impl CoreTypeEntry {
    pub fn get_variant_name(&self) -> &str {
        match self {
            CoreTypeEntry::Holon(e) => e.type_name.as_str(),
            CoreTypeEntry::Property(e) => e.property_type_name.as_str(),
            CoreTypeEntry::String(e) => e.type_name.as_str(),
            CoreTypeEntry::Integer(e) => e.type_name.as_str(),
            CoreTypeEntry::Boolean(e) => e.type_name.as_str(),
            CoreTypeEntry::Enum(e) => e.type_name.as_str(),
            CoreTypeEntry::EnumVariant(e) => e.type_name.as_str(),
            CoreTypeEntry::Relationship(e) => e.relationship_type_name.as_str(),
        }
    }

    pub fn get_type_kind(&self) -> String {
        match self {
            CoreTypeEntry::Holon(_) => "Holon",
            CoreTypeEntry::Property(_) => "Property",
            CoreTypeEntry::String(_) => "String",
            CoreTypeEntry::Integer(_) => "Integer",
            CoreTypeEntry::Boolean(_) => "Boolean",
            CoreTypeEntry::Enum(_) => "Enum",
            CoreTypeEntry::EnumVariant(_) => "EnumVariant",
            CoreTypeEntry::Relationship(_) => "Relationship",
        }
        .to_string()
    }
}
