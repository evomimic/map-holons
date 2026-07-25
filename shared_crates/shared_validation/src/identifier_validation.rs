//! Pure, descriptor-independent validation for Integrity-visible native identifiers.
//!
//! This module owns the role and byte-shape checks that can be expressed over
//! `integrity_core_types` without importing Holochain. For hash-shaped identifiers,
//! the pure layer verifies the expected native width; `holons_guest_integrity`
//! completes the contract by parsing the bytes as the exact Holochain hash type.
//!
//! Keeping those responsibilities separate makes these helpers reusable by
//! coordinator preflight while preserving `shared_validation` as a WASM-safe,
//! substrate-independent crate.

use core_types::HOLOCHAIN_ACTION_HASH_BYTES;
use integrity_core_types::{HolonNodeModel, LocalId, PvlViolation};

/// Fixed structured-diagnostic token for an ActionHash-shaped `LocalId`.
///
/// The pure shape rule and Holochain substrate adapter share this value so both
/// layers classify failures as the same identifier role.
pub const ACTION_HASH_LOCAL_ID_KIND: &str = "ActionHash-shaped LocalId";

/// Fixed structured-diagnostic reason for a value with the wrong native width.
const INCORRECT_BYTE_LENGTH: &str = "incorrect byte length";

/// Validates the native byte shape required of an ActionHash-shaped `LocalId`.
///
/// This pure rule requires exactly [`HOLOCHAIN_ACTION_HASH_BYTES`] bytes but
/// deliberately does not interpret Holochain prefixes or hash types. Exact
/// `ActionHash` parsing belongs to the substrate adapter.
///
/// Every width mismatch, including an empty or oversized value, is classified
/// as [`PvlViolation::InvalidIdentifier`]. `EmptyIdentifier` and
/// `IdentifierTooLong` apply to opaque bounded identifiers, not to a
/// role-specific hash with one exact native shape.
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

/// Validates identifier fields currently persisted in a [`HolonNodeModel`].
///
/// The current entry model permits an absent `original_id`. When present, the
/// field is an ActionHash-shaped `LocalId` and must pass the pure byte-shape
/// rule. Storage SL2 must revisit this orchestration when `original_id` leaves
/// the persisted entry shape.
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
