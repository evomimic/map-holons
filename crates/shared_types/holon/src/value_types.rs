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
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapString(pub String);
impl fmt::Display for MapString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // format the inner string
        write!(f, "{}", self.0)
    }
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapBoolean(pub bool);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapInteger(pub i64);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapEnumValue(pub MapString);

#[derive(Clone, PartialEq, Eq)]
pub struct MapBytes(pub Vec<u8>);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, new)]
pub enum BaseValue {
    StringValue(MapString),
    BooleanValue(MapBoolean),
    IntegerValue(MapInteger),
    EnumValue(MapEnumValue), // this is for simple enum variants,
}

impl BaseValue {
    pub fn into_bytes(&self) -> MapBytes {
        // let string: String = self.into();
        // MapBytes(string.into_bytes())
        match self {
            Self::StringValue(map_string) => MapBytes(map_string.0.clone().into_bytes()),
            Self::BooleanValue(map_bool) => MapBytes(vec![map_bool.0 as u8]),
            Self::IntegerValue(map_int) => MapBytes(vec![map_int.0 as u8]),
            Self::EnumValue(map_enum) => MapBytes(map_enum.0 .0.clone().into_bytes()),
        }
    }
}

// impl TryInto<String> for BaseValue {
//     type Error = ();

//     fn try_into(self) -> Result<String, Self::Error> {
//         match self {
//             BaseValue::StringValue(val) => Ok(val.0.clone()),
//             BaseValue::IntegerValue(val) => Ok(val.0.to_string()),
//             BaseValue::BooleanValue(val) => Ok(val.0.to_string()),
//             BaseValue::EnumValue(val) => Ok(val.0 .0.clone()), // Assuming EnumValue contains a String
//         }
//     }
// }

// using into for conevenience since there is no error case yet

impl Into<String> for &BaseValue {
    fn into(self) -> String {
        match self {
            BaseValue::StringValue(val) => val.0.clone(),
            BaseValue::IntegerValue(val) => val.0.to_string(),
            BaseValue::BooleanValue(val) => val.0.to_string(),
            BaseValue::EnumValue(val) => val.0 .0.clone(), // Assuming EnumValue contains a String
        }
    }
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct EnumValue(pub String);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum BaseType {
    Holon,
    Collection,
    Property,
    Relationship,
    // Boolean,
    // Integer,
    // String,
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
            BaseType::Property => write!(f, "Property"),
            BaseType::Relationship => write!(f, "Relationship"),
            // BaseType::Boolean => write!(f, "Boolean"),
            // BaseType::Integer => write!(f, "Integer"),
            // BaseType::String => write!(f, "String"),
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
