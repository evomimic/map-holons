use derive_new::new;
use hdi::prelude::*;
use std::fmt;

/// The MAP Value Type System is INTENDED to have  three layers.
/// 1) Rust Layer: consisting of a subset of basic Rust datatypes
/// 2) Tuple Structs Layer: Tuple Structs that wrap the Rust following the newtype pattern
/// 3) Enum Layer: defines the BaseValue Enum that includes variants for each of the MAP Value Types.
///     and associates the variant with its corresponding tuple struct
/// The Tuple Structs layer:
///      * encapsulates the choice of Rust type
///      * allows values to be declared without having to explicitly reference the Rust type

/// HOWEVER... for now we are using TypeAliases at level 2, instead of TupleStruct

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct MapString(pub String);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapBoolean(pub bool);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapInteger(pub i64);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapEnumValue(pub MapString);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, new)]
pub enum BaseValue {
    StringValue(MapString),
    BooleanValue(MapBoolean),
    IntegerValue(MapInteger),
    EnumValue(MapEnumValue), // this is for simple enum variants,
}

// TODO: Upgrade MAP Value Type System from type aliases to TupleStructs following newtype pattern

// pub struct MapString (String);
// impl MapString {
//     pub fn to_string(&self)->String {
//         self.0.clone();
//     }
// }

// pub struct MapInteger (i64);
// impl MapInteger {
//     pub fn to_i64(&self)->i64 {
//         self.0;
//     }
// }

// pub struct MapBoolean(bool);
// impl MapBoolean{
//     pub fn to_boolean(&self)->bool {
//         self.0;
//     }
// }

// pub struct MapEnumValue(String);
// impl MapEnumValue {
//     pub fn to_string(&self)->String {
//         self.0.to_string();
//     }
// }

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct EnumValue(pub String);

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
            BaseType::Value(value_type) => match value_type {
                ValueType::Boolean => write!(f, "BooleanValue"),
                ValueType::Enum => write!(f, "EnumValue"),
                ValueType::Integer => write!(f, "IntegerValue"),
                ValueType::String => write!(f, "StringValue"),
            },
            BaseType::ValueArray(value_type) => match value_type {
                ValueType::Boolean => write!(f, "Array of BooleanValue"),
                ValueType::Enum => write!(f, "Array of EnumValue"),
                ValueType::Integer => write!(f, "Array of IntegerValue"),
                ValueType::String => write!(f, "Array of StringValue"),
            },
        }
    }
}
