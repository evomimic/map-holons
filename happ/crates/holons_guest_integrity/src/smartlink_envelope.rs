//! Holochain adapter for descriptor-independent SmartLink validation.
//!
//! The shared validator owns Tag v1 parsing and consensus rules. This module
//! adds only the substrate facts that pure validation cannot establish: exact
//! Holochain hash kinds, outbound-proxy hash encoding, and delete-target action
//! resolution.
//!
//! Host and dependency failures remain in the outer [`ExternResult`]. The
//! inner result is a completed deterministic PVL verdict.

use hdi::prelude::*;
use integrity_core_types::PvlViolation;
use shared_validation::{
    validate_link_delete_target, validate_smartlink_envelope, LinkDeleteTargetKind,
    INVALID_ACTION_HASH_ENCODING, SMARTLINK_BASE_ENDPOINT, SMARTLINK_LINK_TARGET_ENDPOINT,
    SMARTLINK_OUTBOUND_PROXY_ENDPOINT,
};

use crate::local_id_from_action_hash;

const ACTION_HASH_KIND: &str = "ActionHash";
const ENTRY_HASH_KIND: &str = "EntryHash";
const EXTERNAL_HASH_KIND: &str = "ExternalHash";
const UNKNOWN_HASH_KIND: &str = "Unknown";

/// Validates a SmartLink create without resolving either endpoint from the DHT.
///
/// Exact Holochain hash-kind classification happens before the substrate-free
/// validator checks raw endpoint shape and decodes the tag. The returned
/// decoded envelope is retained long enough to validate a present outbound
/// proxy, avoiding a second Tag v1 decode.
pub fn validate_smartlink_create(
    base_address: &AnyLinkableHash,
    target_address: &AnyLinkableHash,
    tag: &LinkTag,
) -> ExternResult<Result<(), PvlViolation>> {
    let base_action_hash = match require_action_hash(base_address, SMARTLINK_BASE_ENDPOINT) {
        Ok(hash) => hash,
        Err(violation) => return Ok(Err(violation)),
    };
    let target_action_hash =
        match require_action_hash(target_address, SMARTLINK_LINK_TARGET_ENDPOINT) {
            Ok(hash) => hash,
            Err(violation) => return Ok(Err(violation)),
        };

    let base_local_id = local_id_from_action_hash(base_action_hash);
    let target_local_id = local_id_from_action_hash(target_action_hash);
    let decoded = match validate_smartlink_envelope(&base_local_id, &target_local_id, tag.as_ref())
    {
        Ok(decoded) => decoded,
        Err(violation) => return Ok(Err(violation)),
    };

    if let Some(external_id) = decoded.target_id.external_id() {
        let outbound_proxy_bytes = external_id.space_id.0.as_bytes().to_vec();
        if ActionHash::try_from_raw_39(outbound_proxy_bytes).is_err() {
            return Ok(Err(PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_OUTBOUND_PROXY_ENDPOINT.into(),
                reason: INVALID_ACTION_HASH_ENCODING.into(),
            }));
        }
    }

    Ok(Ok(()))
}

/// Resolves and classifies the action named by a `DeleteLink`.
///
/// Exactly one [`must_get_action`] supplies every fact needed for classification
/// and later scoped link-type dispatch. An unresolved dependency remains an
/// outer host error and therefore cannot become a permanent PVL rejection.
pub fn resolve_link_delete_target(
    original_action_hash: ActionHash,
) -> ExternResult<Result<CreateLink, PvlViolation>> {
    let target_action = must_get_action(original_action_hash)?;
    let target_kind = match target_action.action() {
        Action::CreateLink(_) => LinkDeleteTargetKind::CreateLink,
        Action::DeleteLink(_) => LinkDeleteTargetKind::DeleteLink,
        _ => LinkDeleteTargetKind::Other,
    };
    if let Err(violation) = validate_link_delete_target(target_kind) {
        return Ok(Err(violation));
    }

    let Action::CreateLink(create_link) = target_action.action() else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "link-delete target classification accepted a non-CreateLink action".into()
        )));
    };

    Ok(Ok(create_link.clone()))
}

/// Validates deletion of an already-valid SmartLink create action.
///
/// Structural validity was established when the original create was accepted,
/// so deletion neither resolves dependencies nor decodes the original tag.
pub fn validate_smartlink_delete(
    _original_action: &CreateLink,
) -> ExternResult<Result<(), PvlViolation>> {
    Ok(Ok(()))
}

fn require_action_hash(hash: &AnyLinkableHash, endpoint: &str) -> Result<ActionHash, PvlViolation> {
    hash.clone().into_action_hash().ok_or_else(|| PvlViolation::UnsupportedSmartLinkEndpointKind {
        endpoint: endpoint.into(),
        endpoint_kind: any_linkable_hash_kind(hash).into(),
    })
}

fn any_linkable_hash_kind(hash: &AnyLinkableHash) -> &'static str {
    if hash.clone().into_action_hash().is_some() {
        ACTION_HASH_KIND
    } else if hash.clone().into_entry_hash().is_some() {
        ENTRY_HASH_KIND
    } else if hash.clone().into_external_hash().is_some() {
        EXTERNAL_HASH_KIND
    } else {
        UNKNOWN_HASH_KIND
    }
}
