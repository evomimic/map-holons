use derive_new::new;
use serde::{Deserialize, Serialize};
use std::fmt;

// Scalar Wrapper Types – newtype wrappers around primitive Rust types
//  (e.g., `MapString`, `MapBoolean`, `MapInteger`, `MapEnumValue`, `MapBytes`)
//  that support serialization, hashing, and consistent formatting.

// ===============================
// 📦 MapString
// ===============================
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, new)]
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
