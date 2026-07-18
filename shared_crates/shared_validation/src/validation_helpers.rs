use core_types::{decode_smartlink_tag, ValidationError, HOLOCHAIN_ACTION_HASH_BYTES};
use integrity_core_types::{HolonNodeModel, LocalId, PersistenceLinkTag};

/// Foundational routines for property and relationship checks
/// applicable to both zomes

// ==== Entry CUD ====

pub fn validate_create_holon(_holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    // Deferring logic until Descriptors

    Ok(())
}

pub fn validate_update_holon(_holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_delete_holon() -> Result<(), ValidationError> {
    Ok(())
}

// ==== Smartlink ====

pub fn validate_create_smartlink_helper(
    base_address: LocalId,
    target_address: LocalId,
    tag: PersistenceLinkTag,
) -> Result<(), ValidationError> {
    validate_smartlink_identity("base address", &base_address)?;
    validate_smartlink_identity("target address", &target_address)?;

    decode_smartlink_tag(&tag.0, target_address).map_err(|error| {
        ValidationError::RelationshipError(format!("Invalid SmartLink Tag v1: {error}"))
    })?;

    Ok(())
}

fn validate_smartlink_identity(field: &str, identity: &LocalId) -> Result<(), ValidationError> {
    if identity.0.len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(ValidationError::RelationshipError(format!(
            "Invalid SmartLink {field}: expected {HOLOCHAIN_ACTION_HASH_BYTES} bytes, got {}",
            identity.0.len()
        )));
    }
    Ok(())
}

pub fn validate_delete_smartlink_helper(
    _base: LocalId,
    _target: LocalId,
) -> Result<(), ValidationError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use base_types::{BaseValue, MapBoolean, MapString};
    use core_types::{
        encode_smartlink_tag, CanonicalKey, HolonId, OccurrenceId, PropertyMap, PropertyName,
        RelationshipName, SmartLinkTagInput, TargetPropertyCacheCandidate,
        MAP_SMARTLINK_V1_MAX_BYTES,
    };

    use super::*;

    fn local_id(seed: u8) -> LocalId {
        LocalId(vec![seed; HOLOCHAIN_ACTION_HASH_BYTES])
    }

    fn valid_tag() -> Vec<u8> {
        valid_tag_with_occurrence(None)
    }

    fn valid_tag_with_occurrence(occurrence_id: Option<OccurrenceId>) -> Vec<u8> {
        encode_smartlink_tag(&SmartLinkTagInput {
            target_id: HolonId::Local(local_id(2)),
            relationship_name: RelationshipName(MapString("Contains".to_string())),
            canonical_key: CanonicalKey::new("target-key").unwrap(),
            occurrence_id,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: vec![TargetPropertyCacheCandidate {
                property_name: PropertyName(MapString("Enabled".to_string())),
                value: BaseValue::BooleanValue(MapBoolean(true)),
            }],
        })
        .unwrap()
    }

    fn validate(tag: Vec<u8>) -> Result<(), ValidationError> {
        validate_create_smartlink_helper(local_id(1), local_id(2), PersistenceLinkTag(tag))
    }

    #[test]
    fn accepts_structurally_valid_v1_tag() {
        assert_eq!(validate(valid_tag()), Ok(()));
    }

    #[test]
    fn accepts_defined_occurrence_flag_and_identity() {
        let tag = valid_tag_with_occurrence(Some(OccurrenceId([7; 16])));
        assert_eq!(validate(tag), Ok(()));
    }

    #[test]
    fn rejects_invalid_base_and_target_identity_widths() {
        let tag = PersistenceLinkTag(valid_tag());
        let base_error = validate_create_smartlink_helper(
            LocalId(vec![1; HOLOCHAIN_ACTION_HASH_BYTES - 1]),
            local_id(2),
            tag.clone(),
        )
        .unwrap_err();
        assert!(base_error.to_string().contains("base address"));

        let target_error = validate_create_smartlink_helper(
            local_id(1),
            LocalId(vec![2; HOLOCHAIN_ACTION_HASH_BYTES + 1]),
            tag,
        )
        .unwrap_err();
        assert!(target_error.to_string().contains("target address"));
    }

    #[test]
    fn rejects_malformed_prefix_version_and_flags() {
        let valid = valid_tag();
        let relationship_delimiter = valid.iter().position(|byte| *byte == 0).unwrap();
        let key_delimiter = valid[relationship_delimiter + 1..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| relationship_delimiter + 1 + offset)
            .unwrap();

        let mut bad_header = valid.clone();
        bad_header[0] ^= 0xff;
        assert_invalid_reason(bad_header, "header");

        assert_invalid_reason(valid[..relationship_delimiter].to_vec(), "delimiter");

        let mut bad_version = valid.clone();
        bad_version[key_delimiter + 1] = 2;
        assert_invalid_reason(bad_version, "version");

        let mut bad_flags = valid;
        bad_flags[key_delimiter + 2] = 0x80;
        assert_invalid_reason(bad_flags, "reserved bits");
    }

    #[test]
    fn rejects_truncated_routing_and_malformed_sections_and_scalars() {
        let mut truncated_proxy = valid_tag();
        let flags_index = payload_flags_index(&truncated_proxy);
        truncated_proxy[flags_index] = 1;
        truncated_proxy.truncate(flags_index + 2);
        assert_invalid_reason(truncated_proxy, "outbound proxy id");

        let mut unknown_section = minimal_tag();
        unknown_section.extend_from_slice(&[3, 0, 1, 0]);
        assert_invalid_reason(unknown_section, "unknown SmartLink property section");

        let mut empty_section = minimal_tag();
        empty_section.extend_from_slice(&[1, 0, 0]);
        assert_invalid_reason(empty_section, "is empty");

        let mut crossing_section = minimal_tag();
        crossing_section.extend_from_slice(&[1, 0, 4, 0]);
        assert_invalid_reason(crossing_section, "section boundary");

        let mut invalid_boolean = minimal_tag();
        invalid_boolean.extend_from_slice(&[2, 0, 7, 0, 1, b'B', 2, 0, 1, 2]);
        assert_invalid_reason(invalid_boolean, "boolean");
    }

    #[test]
    fn rejects_tags_above_the_v1_validity_ceiling() {
        assert_invalid_reason(vec![0; MAP_SMARTLINK_V1_MAX_BYTES + 1], "maximum");
    }

    fn minimal_tag() -> Vec<u8> {
        let tag = valid_tag();
        tag[..payload_flags_index(&tag) + 1].to_vec()
    }

    fn payload_flags_index(tag: &[u8]) -> usize {
        let relationship_delimiter = tag.iter().position(|byte| *byte == 0).unwrap();
        let key_delimiter = tag[relationship_delimiter + 1..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| relationship_delimiter + 1 + offset)
            .unwrap();
        key_delimiter + 2
    }

    fn assert_invalid_reason(tag: Vec<u8>, reason: &str) {
        let error = validate(tag).expect_err("malformed SmartLink tag should be rejected");
        let message = error.to_string();
        assert!(message.contains(reason), "{message:?} did not contain {reason:?}");
    }
}
