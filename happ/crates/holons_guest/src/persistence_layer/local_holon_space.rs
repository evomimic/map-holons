//! Bootstrap index write for the designated LocalHolonSpace lineage root.
//!
//! This is an internal storage operation, not a coordinator API: the canonical path, link type, and
//! empty tag are fixed here so no caller can select infrastructure topology.
//!
//! The sibling `AllHolonNodes` infrastructure index remains owned by
//! `persist_holon(PublishRoot)` in `holon_storage`.

use core_types::HolonError;
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, LOCAL_HOLON_SPACE_PATH};
use holons_integrity::LinkTypes;
use integrity_core_types::LocalId;

/// Anchors the canonical LocalHolonSpace path to a persisted lineage-root HolonNode.
///
/// The caller supplies only a semantic `LocalId`; Integrity independently requires that it resolve
/// to a HolonNode lineage-root `Create`.
pub fn index_local_holon_space(lineage_root_id: &LocalId) -> Result<(), HolonError> {
    let base = Path::from(LOCAL_HOLON_SPACE_PATH)
        .path_entry_hash()
        .map_err(holon_error_from_wasm_error)?;
    let target = try_action_hash_from_local_id(lineage_root_id)?;

    create_link(base, target, LinkTypes::LocalHolonSpace, ())
        .map_err(holon_error_from_wasm_error)?;

    Ok(())
}
