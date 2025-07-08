use crate::HolonNode;
use core_types::HolonError;
use hdi::prelude::*;
use integrity_core_types::{HolonNodeModel, LocalId};

/// Converts a Holochain `ActionHash` into a `LocalId` (raw 39-byte format).
pub fn local_id_from_action_hash(h: ActionHash) -> LocalId {
    LocalId(h.get_raw_39().to_vec())
}

pub fn holon_error_from_wasm_error(error: WasmError) -> HolonError {
    HolonError::WasmError(error.to_string())
}

/// Attempts to convert a `LocalId` back into a Holochain `ActionHash`.
/// Fails if the bytes are not exactly 39 bytes or malformed.
pub fn try_action_hash_from_local_id(id: &LocalId) -> Result<ActionHash, HolonError> {
    ActionHash::try_from_raw_39(id.0.clone())
        .map_err(|e| wasm_error!(WasmErrorInner::Guest(format!("Invalid ActionHash: {}", e))))
        .map_err(|e| holon_error_from_wasm_error(e))
}

/// Converts a guest-side HolonNode entry into the shared model.
impl From<HolonNode> for HolonNodeModel {
    fn from(entry: HolonNode) -> Self {
        HolonNodeModel {
            original_id: entry.original_id.map(Into::into),
            property_map: entry.property_map,
        }
    }
}

/// Converts the shared model into a guest-side HolonNode entry.
impl From<HolonNodeModel> for HolonNode {
    fn from(entry: HolonNodeModel) -> Self {
        HolonNode {
            original_id: entry.original_id.map(Into::into),
            property_map: entry.property_map,
        }
    }
}