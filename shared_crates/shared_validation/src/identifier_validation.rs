//! Descriptor-independent validation for Integrity-visible native identifiers.

use core_types::HOLOCHAIN_ACTION_HASH_BYTES;
use integrity_core_types::{HolonNodeModel, LocalId, PvlViolation};

const ACTION_HASH_LOCAL_ID_KIND: &str = "ActionHash-shaped LocalId";
const INCORRECT_BYTE_LENGTH: &str = "incorrect byte length";

/// Validates the pure, substrate-independent shape of an ActionHash-shaped `LocalId`.
pub fn validate_action_hash_local_id(
    field_name: &str,
    local_id: &LocalId,
) -> Result<(), PvlViolation> {
    if local_id.as_bytes().len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(PvlViolation::InvalidIdentifier {
            field_name: field_name.into(),
            identifier_kind: ACTION_HASH_LOCAL_ID_KIND.into(),
            reason: INCORRECT_BYTE_LENGTH.into(),
        });
    }

    Ok(())
}

/// Validates the identifier fields currently persisted in a `HolonNodeModel`.
pub fn validate_holon_node_identifiers(model: &HolonNodeModel) -> Result<(), PvlViolation> {
    if let Some(original_id) = &model.original_id {
        validate_action_hash_local_id("original_id", original_id)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use integrity_core_types::PropertyMap;

    use super::*;

    fn expected_invalid_original_id() -> PvlViolation {
        PvlViolation::InvalidIdentifier {
            field_name: "original_id".into(),
            identifier_kind: "ActionHash-shaped LocalId".into(),
            reason: "incorrect byte length".into(),
        }
    }

    #[test]
    fn absent_original_id_is_valid() {
        let model = HolonNodeModel::new(None, PropertyMap::new());

        assert_eq!(validate_holon_node_identifiers(&model), Ok(()));
    }

    #[test]
    fn exact_action_hash_width_is_accepted_without_interpreting_the_bytes() {
        let local_id = LocalId(vec![0; HOLOCHAIN_ACTION_HASH_BYTES]);

        assert_eq!(validate_action_hash_local_id("original_id", &local_id), Ok(()));
    }

    #[test]
    fn every_action_hash_length_mismatch_is_an_invalid_identifier() {
        for length in [0, 38, 40, 4_096] {
            let local_id = LocalId(vec![0; length]);

            assert_eq!(
                validate_action_hash_local_id("original_id", &local_id),
                Err(expected_invalid_original_id()),
                "unexpected classification for {length} bytes"
            );
        }
    }

    #[test]
    fn invalid_identifier_uses_the_stable_consensus_code_and_message() {
        let violation = validate_action_hash_local_id("original_id", &LocalId(Vec::new()))
            .expect_err("an empty ActionHash-shaped LocalId must be rejected");

        assert_eq!(violation.code(), "MAP-PVL-1201");
        assert_eq!(violation.to_string(), "MAP-PVL-1201: identifier is invalid");
    }
}
