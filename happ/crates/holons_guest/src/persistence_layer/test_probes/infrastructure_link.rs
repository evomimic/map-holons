//! Infrastructure-link probe for the authoritative `AllHolonNodes` delete policy.

use crate::persistence_layer::holon_storage_externs::to_wasm;
use hdk::prelude::*;
use holons_guest_integrity::{local_id_from_action_hash, try_action_hash_from_local_id};
use holons_integrity::LinkTypes;
use integrity_core_types::LocalId;

/// Deletes only a resolved `AllHolonNodes` create-link action.
///
/// The probe refuses every other action and link type, preserving the narrowest possible ingress
/// for the conductor rejection test. Not a supported write path.
#[hdk_extern]
pub fn all_holon_nodes_delete_for_test(create_link_id: LocalId) -> ExternResult<LocalId> {
    let create_link_hash = try_action_hash_from_local_id(&create_link_id).map_err(to_wasm)?;
    let record = get(create_link_hash.clone(), GetOptions::default())?.ok_or_else(|| {
        wasm_error!(WasmErrorInner::Guest("AllHolonNodes test delete target does not exist".into()))
    })?;
    let Action::CreateLink(create_link) = record.action() else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "AllHolonNodes test delete target is not a CreateLink".into()
        )));
    };
    let Some(LinkTypes::AllHolonNodes) =
        LinkTypes::from_type(create_link.zome_index, create_link.link_type)?
    else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "AllHolonNodes test delete target has another link type".into()
        )));
    };

    let delete_hash = delete_link(create_link_hash, GetOptions::default())?;
    Ok(local_id_from_action_hash(delete_hash))
}
