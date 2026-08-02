//! HolonNode probes for topology and envelope rules unreachable through canonical persistence.

use crate::persistence_layer::holon_storage_externs::to_wasm;
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, HolonNode};
use holons_integrity::EntryTypes;
use integrity_core_types::{HolonNodeModel, LocalId};

/// Authors an `Update` against an exact action hash, with no lineage resolution.
///
/// Used by the sweettest that proves integrity rejects an update aimed at another update rather
/// than at a lineage-root `Create`, and by its companion that proves the same seam is *accepted*
/// when aimed at a root — together showing the rejection is about topology, not about this
/// function.
///
/// Not a supported write path.
#[hdk_extern]
pub fn holon_storage_author_update_for_test(
    input: (LocalId, HolonNodeModel),
) -> ExternResult<LocalId> {
    let (target_id, holon_node) = input;
    let target_hash = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;

    let action_hash =
        update_entry(target_hash, &EntryTypes::HolonNode(HolonNode::from(holon_node)))?;

    Ok(local_id_from_action_hash(action_hash))
}

/// Authors a raw `HolonNode` create without coordinator preflight.
///
/// This probe exists only to prove through a live conductor that Integrity independently rejects
/// a structurally invalid HolonNode after the canonical production boundary begins rejecting the
/// same model before authoring. It fixes the entry type and exposes no generic raw-entry ingress.
///
/// Not a supported write path.
#[hdk_extern]
pub fn holon_storage_author_create_for_test(holon_node: HolonNodeModel) -> ExternResult<LocalId> {
    let action_hash = create_entry(&EntryTypes::HolonNode(HolonNode::from(holon_node)))?;
    Ok(local_id_from_action_hash(action_hash))
}
