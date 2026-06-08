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
    Dance,
    Value(BaseTypeKind),
    ValueArray(BaseTypeKind),
}

impl TypeKind {
    /// Returns the canonical core-schema descriptor key for this kind.
    ///
    /// Value array descriptors currently share one schema key regardless of
    /// their element type, so `ValueArray(_)` intentionally maps lossy.
    pub fn as_schema_key(&self) -> String {
        match self {
            TypeKind::Holon => "TypeKind.Holon".to_string(),
            TypeKind::Collection => "TypeKind.Collection".to_string(),
            TypeKind::Property => "TypeKind.Property".to_string(),
            TypeKind::Relationship => "TypeKind.Relationship".to_string(),
            TypeKind::EnumVariant => "TypeKind.EnumVariant".to_string(),
            TypeKind::Dance => "TypeKind.Dance".to_string(),
            TypeKind::Value(value_type) => match value_type {
                BaseTypeKind::Boolean => "TypeKind.Value.Boolean".to_string(),
                BaseTypeKind::Bytes => "TypeKind.Value.Bytes".to_string(),
                BaseTypeKind::Enum => "TypeKind.Value.Enum".to_string(),
                BaseTypeKind::Integer => "TypeKind.Value.Integer".to_string(),
                BaseTypeKind::String => "TypeKind.Value.String".to_string(),
            },
            TypeKind::ValueArray(_) => "TypeKind.Value.Array".to_string(),
        }
    }
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
            TypeKind::Dance => write!(f, "Dance"),
            TypeKind::Value(value_type) => match value_type {
                BaseTypeKind::Boolean => write!(f, "BooleanValue"),
                BaseTypeKind::Bytes => write!(f, "BytesValue"),
                BaseTypeKind::Enum => write!(f, "EnumValue"),
                BaseTypeKind::Integer => write!(f, "IntegerValue"),
                BaseTypeKind::String => write!(f, "StringValue"),
            },
            TypeKind::ValueArray(value_type) => match value_type {
                BaseTypeKind::Boolean => write!(f, "Array of BooleanValue"),
                BaseTypeKind::Bytes => write!(f, "Array of BytesValue"),
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
    Bytes,
    Enum,
    Integer,
    String,
}

impl fmt::Display for BaseTypeKind {
    /// Displays the `BaseTypeKind` in a readable format.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseTypeKind::Boolean => write!(f, "Boolean"),
            BaseTypeKind::Bytes => write!(f, "Bytes"),
            BaseTypeKind::Enum => write!(f, "Enum"),
            BaseTypeKind::Integer => write!(f, "Integer"),
            BaseTypeKind::String => write!(f, "String"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_kind_display_strings_remain_readable_labels() {
        let cases = [
            (TypeKind::Holon, "Holon"),
            (TypeKind::Collection, "Collection"),
            (TypeKind::Property, "Property"),
            (TypeKind::Relationship, "Relationship"),
            (TypeKind::EnumVariant, "EnumVariant"),
            (TypeKind::Dance, "Dance"),
            (TypeKind::Value(BaseTypeKind::Boolean), "BooleanValue"),
            (TypeKind::Value(BaseTypeKind::Bytes), "BytesValue"),
            (TypeKind::Value(BaseTypeKind::Enum), "EnumValue"),
            (TypeKind::Value(BaseTypeKind::Integer), "IntegerValue"),
            (TypeKind::Value(BaseTypeKind::String), "StringValue"),
            (TypeKind::ValueArray(BaseTypeKind::Boolean), "Array of BooleanValue"),
            (TypeKind::ValueArray(BaseTypeKind::Bytes), "Array of BytesValue"),
            (TypeKind::ValueArray(BaseTypeKind::Enum), "Array of EnumValue"),
            (TypeKind::ValueArray(BaseTypeKind::Integer), "Array of IntegerValue"),
            (TypeKind::ValueArray(BaseTypeKind::String), "Array of StringValue"),
        ];

        for (type_kind, expected) in cases {
            assert_eq!(type_kind.to_string(), expected);
        }
    }

    #[test]
    fn type_kind_schema_keys_match_core_schema_descriptors() {
        let cases = [
            (TypeKind::Holon, "TypeKind.Holon"),
            (TypeKind::Collection, "TypeKind.Collection"),
            (TypeKind::Property, "TypeKind.Property"),
            (TypeKind::Relationship, "TypeKind.Relationship"),
            (TypeKind::EnumVariant, "TypeKind.EnumVariant"),
            (TypeKind::Dance, "TypeKind.Dance"),
            (TypeKind::Value(BaseTypeKind::Boolean), "TypeKind.Value.Boolean"),
            (TypeKind::Value(BaseTypeKind::Bytes), "TypeKind.Value.Bytes"),
            (TypeKind::Value(BaseTypeKind::Enum), "TypeKind.Value.Enum"),
            (TypeKind::Value(BaseTypeKind::Integer), "TypeKind.Value.Integer"),
            (TypeKind::Value(BaseTypeKind::String), "TypeKind.Value.String"),
            (TypeKind::ValueArray(BaseTypeKind::Boolean), "TypeKind.Value.Array"),
            (TypeKind::ValueArray(BaseTypeKind::Bytes), "TypeKind.Value.Array"),
            (TypeKind::ValueArray(BaseTypeKind::Enum), "TypeKind.Value.Array"),
            (TypeKind::ValueArray(BaseTypeKind::Integer), "TypeKind.Value.Array"),
            (TypeKind::ValueArray(BaseTypeKind::String), "TypeKind.Value.Array"),
        ];

        for (type_kind, expected) in cases {
            assert_eq!(type_kind.as_schema_key(), expected);
        }
    }

    #[test]
    fn base_type_kind_display_strings_remain_readable_labels() {
        let cases = [
            (BaseTypeKind::Boolean, "Boolean"),
            (BaseTypeKind::Bytes, "Bytes"),
            (BaseTypeKind::Enum, "Enum"),
            (BaseTypeKind::Integer, "Integer"),
            (BaseTypeKind::String, "String"),
        ];

        for (base_type_kind, expected) in cases {
            assert_eq!(base_type_kind.to_string(), expected);
        }
    }
}
