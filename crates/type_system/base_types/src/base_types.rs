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

impl From<&str> for MapString {
    fn from(s: &str) -> Self {
        MapString(s.to_string())
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
    /// Convert any `BaseValue` to raw bytes (big-endian for integers).
    pub fn into_bytes(&self) -> MapBytes {
        match self {
            Self::StringValue(map_string) => MapBytes(map_string.0.clone().into_bytes()),
            Self::BooleanValue(map_bool) => MapBytes(vec![map_bool.0 as u8]),
            Self::IntegerValue(map_int) => MapBytes(map_int.0.to_be_bytes().to_vec()),
            Self::EnumValue(map_enum) => MapBytes(map_enum.0 .0.clone().into_bytes()),
        }
    }
}

/// Convert a `&BaseValue` to a `String` for display-like usage.
/// (This is intentionally *not* a full lossless conversion for all variants.)
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
// 🔀 Canonical conversion API
// ===============================

/// Preferred, explicit conversion into `BaseValue`.
///
/// Import the trait (usually via the prelude) and call:
/// - `"hello".to_base_value()`
/// - `MapString("x".into()).to_base_value()`
/// - `42_i64.to_base_value()`
/// - `true.to_base_value()`
/// - `some_base_value.to_base_value()` (no-op)
pub trait ToBaseValue {
    fn to_base_value(self) -> BaseValue;
}

// Owned wrappers → BaseValue
impl ToBaseValue for MapString {
    fn to_base_value(self) -> BaseValue { BaseValue::StringValue(self) }
}
impl ToBaseValue for MapBoolean {
    fn to_base_value(self) -> BaseValue { BaseValue::BooleanValue(self) }
}
impl ToBaseValue for MapInteger {
    fn to_base_value(self) -> BaseValue { BaseValue::IntegerValue(self) }
}
impl ToBaseValue for MapEnumValue {
    fn to_base_value(self) -> BaseValue { BaseValue::EnumValue(self) }
}

// References to wrappers → BaseValue (clone as needed)
impl ToBaseValue for &MapString {
    fn to_base_value(self) -> BaseValue { BaseValue::StringValue(self.clone()) }
}
impl ToBaseValue for &MapBoolean {
    fn to_base_value(self) -> BaseValue { BaseValue::BooleanValue(self.clone()) }
}
impl ToBaseValue for &MapInteger {
    fn to_base_value(self) -> BaseValue { BaseValue::IntegerValue(self.clone()) }
}
impl ToBaseValue for &MapEnumValue {
    fn to_base_value(self) -> BaseValue { BaseValue::EnumValue(self.clone()) }
}

// Primitives → BaseValue
impl ToBaseValue for &str {
    fn to_base_value(self) -> BaseValue { BaseValue::StringValue(MapString(self.to_string())) }
}
impl ToBaseValue for String {
    fn to_base_value(self) -> BaseValue { BaseValue::StringValue(MapString(self)) }
}
impl ToBaseValue for bool {
    fn to_base_value(self) -> BaseValue { BaseValue::BooleanValue(MapBoolean(self)) }
}
impl ToBaseValue for i64 {
    fn to_base_value(self) -> BaseValue { BaseValue::IntegerValue(MapInteger(self)) }
}

// Identity conversions
impl ToBaseValue for BaseValue {
    #[inline]
    fn to_base_value(self) -> BaseValue { self }
}
impl ToBaseValue for &BaseValue {
    #[inline]
    fn to_base_value(self) -> BaseValue { self.clone() }
}

// ===============================
// 🧰 Convenience conversions (wrappers <-> primitives)
// These do NOT convert to BaseValue; they’re general ergonomics that don’t
// conflict with the ToBaseValue API.
// ===============================

// MapString
impl From<String> for MapString {
    #[inline]
    fn from(value: String) -> Self { MapString(value) }
}
impl From<&str> for MapString {
    #[inline]
    fn from(value: &str) -> Self { MapString(value.to_owned()) }
}

// MapBoolean
impl From<bool> for MapBoolean {
    #[inline]
    fn from(value: bool) -> Self { MapBoolean(value) }
}

// MapInteger <-> i64
impl From<i64> for MapInteger {
    #[inline]
    fn from(value: i64) -> Self { MapInteger(value) }
}
impl From<MapInteger> for i64 {
    #[inline]
    fn from(value: MapInteger) -> Self { value.0 }
}