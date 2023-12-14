use hdi::prelude::*;
use derive_new::new;
use std::collections::btree_map::BTreeMap;
use std::fmt;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}
pub type HolonId = ActionHash;
pub type PropertyName = String;
pub type PropertyMap = BTreeMap<PropertyName, BaseValue>;
/// BaseValue types are deliberately kept fairly primitive for now
/// More complex requirements are addressed via Holons and HolonRelationships
///

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, new)]
pub enum BaseValue {
    StringValue(String),
    BooleanValue(bool),
    IntegerValue(i64),
    EnumValue(EnumValue), // this is for simple enum variants,
}
// #[hdk_entry_helper]
// #[derive(new, Clone, PartialEq, Eq)]
pub type EnumValue = String;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum BaseType {
    Holon,
    Collection,
    Relationship,
    Boolean,
    Integer,
    String,
    Value(ValueType),
    ValueArray(ValueType),
}
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum ValueType {
    Boolean,
    Enum,
    Integer,
    String,
}



impl fmt::Display for BaseType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BaseType::Holon => write!(f, "Holon"),
            BaseType::Collection => write!(f, "Collection"),
            //BaseType::Composite => write!(f, "Composite"),
            BaseType::Relationship => write!(f, "Relationship"),
            BaseType::Boolean => write!(f, "Boolean"),
            BaseType::Integer => write!(f, "Integer"),
            BaseType::String => write!(f, "String"),
            // BaseType::EnumValue => write!(f, "EnumValue"),
            // BaseType::EnumHolon => write!(f, "EnumHolon"),
            BaseType::Value(value_type) => {
                match value_type {
                    ValueType::Boolean => write!(f, "BooleanValue"),
                    ValueType::Enum => write!(f, "EnumValue"),
                    ValueType::Integer => write!(f, "IntegerValue"),
                    ValueType::String => write!(f, "StringValue"),
                }
            },
            BaseType::ValueArray(value_type) => {
                match value_type {
                    ValueType::Boolean => write!(f, "Array of BooleanValue"),
                    ValueType::Enum => write!(f, "Array of EnumValue"),
                    ValueType::Integer => write!(f, "Array of IntegerValue"),
                    ValueType::String => write!(f, "Array of StringValue"),
                }
            }
        }
    }
}

// #[hdk_entry_helper]
// #[derive(new, Clone, PartialEq, Eq, PartialOrd, Ord)]
// pub struct SemanticVersion {
//     major: u8,
//     minor: u8,
//     patch: u8,
// }

// impl Default for SemanticVersion {
//     fn default() -> Self {
//         SemanticVersion {
//             major: 0,
//             minor: 0,
//             patch: 1,
//         }
//     }
// }
