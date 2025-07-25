use crate::HolonNode;
use core_types::HolonError;
use hdi::prelude::*;
use integrity_core_types::{HolonNodeModel, LocalId, PersistenceAgentId};


pub fn holon_error_from_wasm_error(error: WasmError) -> HolonError {
    HolonError::WasmError(error.to_string())
}


/// Converts a Holochain `ActionHash` into a `LocalId` (raw 39-byte format).
pub fn local_id_from_action_hash(h: ActionHash) -> LocalId {
    LocalId(h.get_raw_39().to_vec())
}

/// Converts a Holochain `ActionHash` into a `LocalId` (raw 39-byte format).
pub fn persistence_agent_id_from_agent_pub_key(h: AgentPubKey) -> PersistenceAgentId {
    PersistenceAgentId(h.get_raw_39().to_vec())
}



/// Attempts to convert a `LocalId` back into a Holochain `ActionHash`.
/// Fails if the bytes are not exactly 39 bytes or malformed.
pub fn try_action_hash_from_local_id(id: &LocalId) -> Result<ActionHash, HolonError> {
    ActionHash::try_from_raw_39(id.0.clone())
        .map_err(|e| wasm_error!(WasmErrorInner::Guest(format!("Invalid ActionHash: {}", e))))
        .map_err(|e| holon_error_from_wasm_error(e))
}

/// Attempts to convert a `PersistenceAgentId` back into a Holochain `AgentPubkey`.
/// Fails if the bytes are not exactly 39 bytes or malformed.
pub fn try_action_hash_from_persistence_agent_id(id: &PersistenceAgentId) -> Result<AgentPubKey, HolonError> {
    AgentPubKey::try_from_raw_39(id.0.clone())
        .map_err(|e| wasm_error!(WasmErrorInner::Guest(format!("Invalid AgentPubKey: {}", e))))
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