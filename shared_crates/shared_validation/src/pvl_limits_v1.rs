//! Versioned limits and measurement helpers for PVL v1.
//!
//! These constants define Integrity semantics: changing any value changes what
//! every peer accepts and therefore requires deliberate DNA versioning. The
//! values are the initial measurements specified for PVL v1; re-ratification
//! against the Tag v1 encoder is owned by PVL plan PR 9.

/// Maximum canonical serialized byte length of a HolonNode entry.
pub const MAX_HOLON_NODE_BYTES: usize = 262_144;
/// Maximum number of properties in a HolonNode property map.
pub const MAX_PROPERTY_COUNT: usize = 256;
/// Maximum UTF-8 byte length of a property name.
pub const MAX_PROPERTY_NAME_BYTES: usize = 128;
/// Maximum UTF-8 byte length of a string property value.
pub const MAX_STRING_VALUE_BYTES: usize = 16_384;
/// Maximum UTF-8 byte length of an enum property value.
pub const MAX_ENUM_VALUE_BYTES: usize = 256;
/// Maximum serialized byte length of a SmartLink canonical key.
pub const MAX_CANONICAL_KEY_BYTES: usize = 256;
/// Maximum raw byte length of a bytes property value.
pub const MAX_BYTES_VALUE_BYTES: usize = 131_072;
/// Maximum number of items in a collection property value.
pub const MAX_COLLECTION_ITEMS: usize = 1_024;
/// Maximum nesting depth of a property value.
pub const MAX_VALUE_NESTING_DEPTH: usize = 2;
/// Maximum UTF-8 byte length of a relationship name.
pub const MAX_RELATIONSHIP_NAME_BYTES: usize = 128;
/// Maximum serialized byte length of a remote object identifier.
pub const MAX_REMOTE_OBJECT_ID_BYTES: usize = 256;
/// Maximum deterministic validation dependencies requested for one operation.
pub const MAX_VALIDATION_DEPENDENCIES_PER_OP: usize = 8;

/// Codec-owned MAP SmartLink Tag v1 wire-validity ceiling.
pub use core_types::MAP_SMARTLINK_V1_MAX_BYTES;

/// Measures a string using its canonical UTF-8 byte representation.
pub fn utf8_byte_len(value: &str) -> usize {
    value.len()
}

/// Measures a byte value without interpreting or re-encoding it.
pub fn raw_byte_len(value: &[u8]) -> usize {
    value.len()
}

/// Narrows a measured length to `u32` without wrapping.
pub fn saturating_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// Narrows a measured length to `u16` without wrapping.
pub fn saturating_u16(len: usize) -> u16 {
    u16::try_from(len).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_values_match_the_pvl_v1_contract() {
        assert_eq!(MAX_HOLON_NODE_BYTES, 262_144);
        assert_eq!(MAX_PROPERTY_COUNT, 256);
        assert_eq!(MAX_PROPERTY_NAME_BYTES, 128);
        assert_eq!(MAX_STRING_VALUE_BYTES, 16_384);
        assert_eq!(MAX_ENUM_VALUE_BYTES, 256);
        assert_eq!(MAX_CANONICAL_KEY_BYTES, 256);
        assert_eq!(MAX_BYTES_VALUE_BYTES, 131_072);
        assert_eq!(MAX_COLLECTION_ITEMS, 1_024);
        assert_eq!(MAX_VALUE_NESTING_DEPTH, 2);
        assert_eq!(MAX_RELATIONSHIP_NAME_BYTES, 128);
        assert_eq!(MAX_REMOTE_OBJECT_ID_BYTES, 256);
        assert_eq!(MAX_VALIDATION_DEPENDENCIES_PER_OP, 8);
        assert_eq!(MAP_SMARTLINK_V1_MAX_BYTES, 512);
    }

    #[test]
    fn smartlink_ceiling_is_the_codec_owned_constant() {
        assert_eq!(MAP_SMARTLINK_V1_MAX_BYTES, core_types::MAP_SMARTLINK_V1_MAX_BYTES);
    }

    #[test]
    fn measurement_helpers_count_serialized_bytes() {
        assert_eq!(utf8_byte_len("a"), 1);
        assert_eq!(utf8_byte_len("é"), 2);
        assert_eq!(utf8_byte_len("→"), 3);
        assert_eq!(utf8_byte_len("😀"), 4);
        assert_eq!(raw_byte_len(&[0, 1, 2, 255]), 4);
    }

    #[test]
    fn width_conversions_saturate_instead_of_wrapping() {
        assert_eq!(saturating_u16(0), 0);
        assert_eq!(saturating_u16(u16::MAX as usize), u16::MAX);
        assert_eq!(saturating_u16(u16::MAX as usize + 1), u16::MAX);

        assert_eq!(saturating_u32(0), 0);
        assert_eq!(saturating_u32(u32::MAX as usize), u32::MAX);

        #[cfg(target_pointer_width = "64")]
        assert_eq!(saturating_u32(u32::MAX as usize + 1), u32::MAX);
    }
}
