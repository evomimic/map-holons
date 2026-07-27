//! Zome-extern wrappers over the SmartLink storage API (Issue #594).
//!
//! These thin `#[hdk_extern]` shims expose the descriptor-unaware storage functions in
//! `persistence_layer::smartlink` so they can be driven directly from a live-conductor
//! sweettest (bypassing the dance surface). They mirror the always-present raw persistence
//! externs in `holon_node.rs`: owned arguments, `ExternResult` returns, `HolonError` folded
//! into `WasmError` at the boundary. Multi-argument calls take a tuple to avoid introducing
//! a shared transport DTO — the sweettest constructs inputs from already-shared crates only.

use crate::persistence_layer::smartlink::{
    delete_smartlink, expand_all_from_source, expand_from_source, expand_from_source_by_key,
    put_smartlink,
};
use core_types::{
    DeleteSmartLinkOutcome, HolonError, KeyMatch, PreparedSmartLink, PutSmartLinkOutcome,
    SmartLink, SmartLinkId,
};
use hdk::prelude::*;
use integrity_core_types::{LocalId, RelationshipName};

fn to_wasm(error: HolonError) -> WasmError {
    wasm_error!(WasmErrorInner::Guest(error.to_string()))
}

#[hdk_extern]
pub fn smartlink_put(prepared: PreparedSmartLink) -> ExternResult<PutSmartLinkOutcome> {
    put_smartlink(prepared).map_err(to_wasm)
}

#[hdk_extern]
pub fn smartlink_expand_all(source_id: LocalId) -> ExternResult<Vec<SmartLink>> {
    expand_all_from_source(&source_id).map_err(to_wasm)
}

#[hdk_extern]
pub fn smartlink_expand(input: (LocalId, RelationshipName)) -> ExternResult<Vec<SmartLink>> {
    let (source_id, relationship_name) = input;
    expand_from_source(&source_id, &relationship_name).map_err(to_wasm)
}

#[hdk_extern]
pub fn smartlink_expand_by_key(
    input: (LocalId, RelationshipName, KeyMatch),
) -> ExternResult<Vec<SmartLink>> {
    let (source_id, relationship_name, key_match) = input;
    expand_from_source_by_key(&source_id, &relationship_name, key_match).map_err(to_wasm)
}

#[hdk_extern]
pub fn smartlink_delete(smartlink_id: SmartLinkId) -> ExternResult<DeleteSmartLinkOutcome> {
    delete_smartlink(&smartlink_id).map_err(to_wasm)
}
