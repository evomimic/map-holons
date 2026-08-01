//! Zome-extern wrappers over the holon node storage API (Issue #607).
//!
//! These thin `#[hdk_extern]` shims expose the storage functions in
//! `persistence_layer::holon_storage` so they can be driven directly from a live-conductor
//! sweettest, bypassing the dance surface. They mirror the SmartLink externs: owned arguments,
//! `ExternResult` returns, `HolonError` folded into `WasmError` at the boundary, and
//! multi-argument calls taking a tuple so no shared transport DTO has to be invented.
//!
//! Every extern here delegates to the real storage API and adds no behaviour of its own. The
//! deliberately-invalid authoring seam needed by one integrity test is kept out of this module —
//! see `test_probes`.

use crate::persistence_layer::holon_storage::{get_holon, get_holons, persist_holon};
use core_types::{HolonError, HolonWriteRequest, StoredHolonNode};
use hdk::prelude::*;
use integrity_core_types::LocalId;

pub(crate) fn to_wasm(error: HolonError) -> WasmError {
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
