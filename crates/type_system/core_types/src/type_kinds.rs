use serde::{Deserialize, Serialize};
use std::fmt;


// ===============================
// 📦 TypeKind Enum
// ===============================
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum TypeKind {
    Holon,
    Collection,
    Property,
    Relationship,
    EnumVariant,
    Value(BaseTypeKind),
    ValueArray(BaseTypeKind),
}

impl fmt::Display for TypeKind {
    /// Displays the `TypeKind` with clear type labeling.
    ///
    /// Example:
    /// - `Value(Integer)` → `IntegerValue`
    /// - `ValueArray(String)` → `Array of StringValue`
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TypeKind::Holon => write!(f, "Holon"),
            TypeKind::Collection => write!(f, "Collection"),
            TypeKind::Property => write!(f, "Property"),
            TypeKind::Relationship => write!(f, "Relationship"),
            TypeKind::EnumVariant => write!(f, "EnumVariant"),
            TypeKind::Value(value_type) => match value_type {
                BaseTypeKind::Boolean => write!(f, "BooleanValue"),
                BaseTypeKind::Enum => write!(f, "EnumValue"),
                BaseTypeKind::Integer => write!(f, "IntegerValue"),
                BaseTypeKind::String => write!(f, "StringValue"),
            },
            TypeKind::ValueArray(value_type) => match value_type {
                BaseTypeKind::Boolean => write!(f, "Array of BooleanValue"),
                BaseTypeKind::Enum => write!(f, "Array of EnumValue"),
                BaseTypeKind::Integer => write!(f, "Array of IntegerValue"),
                BaseTypeKind::String => write!(f, "Array of StringValue"),
            },
        }
    }
}

// ===============================
// 📦 BaseTypeKind Enum
// ===============================
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum BaseTypeKind {
    Boolean,
    Enum,
    Integer,
    String,
}

impl fmt::Display for BaseTypeKind {
    /// Displays the `BaseTypeKind` in a readable format.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseTypeKind::Boolean => write!(f, "Boolean"),
            BaseTypeKind::Enum => write!(f, "Enum"),
            BaseTypeKind::Integer => write!(f, "Integer"),
            BaseTypeKind::String => write!(f, "String"),
        }
    }
}