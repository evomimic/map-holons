pub use holons_core::*;
pub mod guest {
    pub use holons_guest::*;
}
use hdk::prelude::*;
use holons_guest_integrity::{local_id_from_action_hash, HolonNode};
use holons_integrity::*;
use integrity_core_types::LocalId;

#[hdk_extern]
pub fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Signal {
    LinkCreated { action_id: LocalId, link_type: String },
    LinkDeleted { action_id: LocalId, link_type: String },
    HolonCreated { action_id: LocalId, holon: HolonNode },
    HolonUpdated { action_id: LocalId, holon: HolonNode, original_holon: HolonNode },
    HolonDeleted { action_id: LocalId, original_holon: HolonNode },
}
#[hdk_extern(infallible)]
pub fn post_commit(committed_actions: Vec<SignedActionHashed>) {
    for action in committed_actions {
        if let Err(err) = signal_action(action) {
            error!("Error signaling new action: {:?}", err);
        }
    }
}
fn signal_action(action: SignedActionHashed) -> ExternResult<()> {
    match action.hashed.content.clone() {
        Action::CreateLink(create_link) => {
            if let Ok(Some(link_type)) =
                LinkTypes::from_type(create_link.zome_index, create_link.link_type)
            {
                let action_id = local_id_from_action_hash(action.hashed.hash.clone());
                emit_signal(Signal::LinkCreated {
                    action_id,
                    link_type: format!("{:?}", link_type),
                })?;
            }
            Ok(())
        }
        Action::DeleteLink(delete_link) => {
            let record = get(delete_link.link_add_address.clone(), GetOptions::default())?.ok_or(
                wasm_error!(WasmErrorInner::Guest("Failed to fetch CreateLink action".to_string())),
            )?;
            match record.action() {
                Action::CreateLink(create_link) => {
                    if let Ok(Some(link_type)) =
                        LinkTypes::from_type(create_link.zome_index, create_link.link_type)
                    {
                        let action_id = local_id_from_action_hash(action.hashed.hash.clone());
                        emit_signal(Signal::LinkDeleted {
                            action_id,
                            link_type: format!("{:?}", link_type),
                        })?;
                    }
                    Ok(())
                }
                _ => {
                    return Err(wasm_error!(WasmErrorInner::Guest(
                        "Create Link should exist".to_string()
                    )));
                }
            }
        }
        Action::Create(_create) => {
            if let Ok(Some(EntryTypes::HolonNode(holon))) =
                get_entry_for_action(&action.hashed.hash)
            {
                let action_id = local_id_from_action_hash(action.hashed.hash.clone());
                emit_signal(Signal::HolonCreated { action_id, holon })?;
            }
            Ok(())
        }
        Action::Update(update) => {
            if let Ok(Some(EntryTypes::HolonNode(holon))) =
                get_entry_for_action(&action.hashed.hash)
            {
                if let Ok(Some(EntryTypes::HolonNode(original_holon))) =
                    get_entry_for_action(&update.original_action_address)
                {
                    let action_id = local_id_from_action_hash(action.hashed.hash.clone());
                    emit_signal(Signal::HolonUpdated { action_id, holon, original_holon })?;
                }
            }
            Ok(())
        }
        Action::Delete(delete) => {
            if let Ok(Some(EntryTypes::HolonNode(original_holon))) =
                get_entry_for_action(&delete.deletes_address)
            {
                let action_id = local_id_from_action_hash(action.hashed.hash.clone());
                emit_signal(Signal::HolonDeleted { action_id, original_holon })?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn get_entry_for_action(action_hash: &ActionHash) -> ExternResult<Option<EntryTypes>> {
    let record = match get_details(action_hash.clone(), GetOptions::default())? {
        Some(Details::Record(record_details)) => record_details.record,
        _ => {
            return Ok(None);
        }
    };
    let entry = match record.entry().as_option() {
        Some(entry) => entry,
        None => {
            return Ok(None);
        }
    };
    let (zome_index, entry_index) = match record.action().entry_type() {
        Some(EntryType::App(AppEntryDef { zome_index, entry_index, .. })) => {
            (zome_index, entry_index)
        }
        _ => {
            return Ok(None);
        }
    };
    Ok(EntryTypes::deserialize_from_type(zome_index.clone(), entry_index.clone(), entry)?)
}
