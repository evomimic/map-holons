//! Zome-extern wrappers over the holon node storage API (Issue #607).
//!
//! These thin `#[hdk_extern]` shims expose the storage functions in
//! `persistence_layer::holon_storage` so they can be driven directly from a live-conductor
//! sweettest, bypassing the dance surface. They mirror the SmartLink externs: owned arguments,
//! `ExternResult` returns, `HolonError` folded into `WasmError` at the boundary, and
//! multi-argument calls taking a tuple so no shared transport DTO has to be invented.

use crate::persistence_layer::holon_storage::{get_holon, get_holons, persist_holon};
use core_types::{HolonError, HolonWriteRequest, StoredHolonNode};
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, HolonNode};
use holons_integrity::EntryTypes;
use integrity_core_types::{HolonNodeModel, LocalId};

fn to_wasm(error: HolonError) -> WasmError {
    wasm_error!(WasmErrorInner::Guest(error.to_string()))
}

#[hdk_extern]
pub fn holon_storage_get(local_id: LocalId) -> ExternResult<Option<StoredHolonNode>> {
    get_holon(&local_id).map_err(to_wasm)
}

#[hdk_extern]
pub fn holon_storage_get_many(
    local_ids: Vec<LocalId>,
) -> ExternResult<Vec<Option<StoredHolonNode>>> {
    get_holons(&local_ids).map_err(to_wasm)
}

#[hdk_extern]
pub fn holon_storage_persist(request: HolonWriteRequest) -> ExternResult<StoredHolonNode> {
    persist_holon(request).map_err(to_wasm)
}

/// Authors an `Update` against an exact action hash, with no lineage resolution.
///
/// This exists solely so a sweettest can drive the integrity update-target rule with a topology
/// `persist_holon` refuses to construct: `persist_holon` always addresses the lineage root, so
/// it can never produce an update that targets another update. Integrity is expected to reject
/// what this authors — that rejection is the thing under test.
///
/// Not a supported write path. Nothing outside tests should call it.
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
