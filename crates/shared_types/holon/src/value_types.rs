use derive_new::new;
use hdi::prelude::*;
use std::fmt;

/// The MAP Value Type System is INTENDED to have three layers:
/// 1️⃣ Rust Layer: Basic Rust data types
/// 2️⃣ Tuple Structs Layer: Encapsulates Rust types using the newtype pattern
/// 3️⃣ Enum Layer: `BaseValue` enum representing various MAP Value Types

// ===============================
// 📦 MapString
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapString(pub String);

impl fmt::Display for MapString {
    /// Displays the inner `String` directly.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl MapString {
    pub fn into_base_value(self) -> BaseValue {
        BaseValue::StringValue(self)
    }
}

// ===============================
// 📦 MapBoolean
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapBoolean(pub bool);

impl fmt::Display for MapBoolean {
    /// Displays the boolean as `true` or `false`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// 📦 MapInteger
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapInteger(pub i64);

impl fmt::Display for MapInteger {
    /// Displays the integer in its standard numeric form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// 📦 MapEnumValue
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct MapEnumValue(pub MapString);

impl fmt::Display for MapEnumValue {
    /// Displays the enum value as its inner string representation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// 📦 MapBytes
// ===============================
#[derive(Clone, PartialEq, Eq)]
pub struct MapBytes(pub Vec<u8>);

impl fmt::Display for MapBytes {
    /// Displays the byte vector as a hexadecimal string.
    ///
    /// Example: `MapBytes([1, 2, 3])` → `0x010203`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

// ===============================
// 📦 BaseValue Enum
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, new)]
pub enum BaseValue {
    StringValue(MapString),
    BooleanValue(MapBoolean),
    IntegerValue(MapInteger),
    EnumValue(MapEnumValue), // for simple enum variants
}

impl fmt::Display for BaseValue {
    /// Displays the `BaseValue` in a variant-specific format.
    ///
    /// Examples:
    /// - `StringValue("Hello")`
    /// - `BooleanValue(true)`
    /// - `IntegerValue(42)`
    /// - `EnumValue(Status)`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseValue::StringValue(val) => write!(f, "StringValue(\"{}\")", val),
            BaseValue::BooleanValue(val) => write!(f, "BooleanValue({})", val),
            BaseValue::IntegerValue(val) => write!(f, "IntegerValue({})", val),
            BaseValue::EnumValue(val) => write!(f, "EnumValue({})", val),
        }
    }
}

impl BaseValue {
    pub fn into_bytes(&self) -> MapBytes {
        match self {
            Self::StringValue(map_string) => MapBytes(map_string.0.clone().into_bytes()),
            Self::BooleanValue(map_bool) => MapBytes(vec![map_bool.0 as u8]),
            Self::IntegerValue(map_int) => MapBytes(map_int.0.to_be_bytes().to_vec()),
            Self::EnumValue(map_enum) => MapBytes(map_enum.0 .0.clone().into_bytes()),
        }
    }
}

// ===============================
// 🔀 Conversion Implementations
// ===============================
impl Into<String> for &BaseValue {
    fn into(self) -> String {
        match self {
            BaseValue::StringValue(val) => val.0.clone(),
            BaseValue::IntegerValue(val) => val.0.to_string(),
            BaseValue::BooleanValue(val) => val.0.to_string(),
            BaseValue::EnumValue(val) => val.0 .0.clone(),
        }
    }
}

// ===============================
// 📦 EnumValue
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct EnumValue(pub String);

impl fmt::Display for EnumValue {
    /// Displays the enum value directly as its string content.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// 📦 BaseType Enum
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum BaseType {
    Holon,
    Collection,
    Property,
    Relationship,
    EnumVariant,
    Value(ValueType),
    ValueArray(ValueType),
}

impl fmt::Display for BaseType {
    /// Displays the `BaseType` with clear type labeling.
    ///
    /// Example:
    /// - `Value(Integer)` → `IntegerValue`
    /// - `ValueArray(String)` → `Array of StringValue`
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BaseType::Holon => write!(f, "Holon"),
            BaseType::Collection => write!(f, "Collection"),
            BaseType::Property => write!(f, "Property"),
            BaseType::Relationship => write!(f, "Relationship"),
            BaseType::EnumVariant => write!(f, "EnumVariant"),
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

// ===============================
// 📦 ValueType Enum
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum ValueType {
    Boolean,
    Enum,
    Integer,
    String,
}

impl fmt::Display for ValueType {
    /// Displays the `ValueType` in a readable format.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Boolean => write!(f, "Boolean"),
            ValueType::Enum => write!(f, "Enum"),
            ValueType::Integer => write!(f, "Integer"),
            ValueType::String => write!(f, "String"),
        }
    }
}
