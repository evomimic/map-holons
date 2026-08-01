//! Deliberately-invalid SmartLink authoring seams, for integrity tests only (Issue #618).
//!
//! # This is not part of the storage API
//!
//! These probes exist only where typed production APIs cannot construct the operation needed to
//! exercise a live Integrity rule. They are narrowly fixed to their named link type and must not
//! become generic raw-link authoring or deletion endpoints.
//!
//! # Why it is not feature-gated
//!
//! Sweettests run against the DNA produced by `npm run build:happ`, the same artifact the host
//! ships. Enabling a test feature in that artifact would make the gate decorative, so the probes
//! remain always present, conspicuously named, and unsupported as production ingress.

use crate::persistence_layer::holon_storage_externs::to_wasm;
use hdk::prelude::*;
use holons_guest_integrity::{local_id_from_action_hash, try_action_hash_from_local_id};
use holons_integrity::LinkTypes;
use integrity_core_types::LocalId;

/// Authors caller-supplied Tag v1 bytes under the fixed `SmartLink` type.
///
/// This is the minimum seam needed to prove the Integrity adapter rejects malformed peer-authored
/// bytes that the canonical storage encoder will never produce. Not a supported write path.
#[hdk_extern]
pub fn smartlink_author_raw_tag_for_test(
    input: (LocalId, LocalId, Vec<u8>),
) -> ExternResult<LocalId> {
    let (source_id, target_id, raw_tag) = input;
    let source = try_action_hash_from_local_id(&source_id).map_err(to_wasm)?;
    let target = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;
    let action_hash = create_link(source, target, LinkTypes::SmartLink, LinkTag(raw_tag))?;
    Ok(local_id_from_action_hash(action_hash))
}
