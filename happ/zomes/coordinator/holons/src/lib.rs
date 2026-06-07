pub use holons_core::*;
pub mod guest {
    pub use holons_guest::*;
}
use hdk::prelude::*;
use holons_guest_integrity::local_id_from_action_hash;
use holons_integrity::*;
use integrity_core_types::{LocalId, PersistenceTimestamp};

#[hdk_extern]
pub fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Signal {
    LinkCreated { action_id: LocalId, link_type: String, timestamp: PersistenceTimestamp },
    LinkDeleted { action_id: LocalId, link_type: String, timestamp: PersistenceTimestamp },
    HolonCreated { action_id: LocalId, affected_holon: LocalId, timestamp: PersistenceTimestamp },
    HolonUpdated {
        action_id: LocalId,
        affected_holon: LocalId,
        previous_holon: LocalId,
        timestamp: PersistenceTimestamp,
    },
    HolonDeleted {
        action_id: LocalId,
        affected_holon: LocalId,
        previous_holon: LocalId,
        timestamp: PersistenceTimestamp,
    },
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
    let action_hash = action.hashed.hash.clone();
    let action_id = local_id_from_action_hash(action_hash.clone());
    let timestamp = PersistenceTimestamp(action.hashed.content.timestamp().0);
    match action.hashed.content.clone() {
        Action::CreateLink(create_link) => {
            if let Ok(Some(link_type)) =
                LinkTypes::from_type(create_link.zome_index, create_link.link_type)
            {
                emit_signal(Signal::LinkCreated {
                    action_id,
                    link_type: format!("{:?}", link_type),
                    timestamp,
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
                        emit_signal(Signal::LinkDeleted {
                            action_id,
                            link_type: format!("{:?}", link_type),
                            timestamp,
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
            if let Ok(Some(EntryTypes::HolonNode(_))) = get_entry_for_action(&action_hash) {
                // A freshly created holon's permanent identity is its own create hash.
                emit_signal(Signal::HolonCreated {
                    action_id: action_id.clone(),
                    affected_holon: action_id,
                    timestamp,
                })?;
            }
            Ok(())
        }
        Action::Update(update) => {
            if let Ok(Some(EntryTypes::HolonNode(holon))) = get_entry_for_action(&action_hash) {
                // affected_holon = permanent lineage id; previous_holon = the superseded record.
                let affected_holon = holon.original_id.clone().unwrap_or_else(|| action_id.clone());
                let previous_holon =
                    local_id_from_action_hash(update.original_action_address.clone());
                emit_signal(Signal::HolonUpdated {
                    action_id,
                    affected_holon,
                    previous_holon,
                    timestamp,
                })?;
            }
            Ok(())
        }
        Action::Delete(delete) => {
            if let Ok(Some(EntryTypes::HolonNode(original_holon))) =
                get_entry_for_action(&delete.deletes_address)
            {
                let previous_holon = local_id_from_action_hash(delete.deletes_address.clone());
                let affected_holon =
                    original_holon.original_id.clone().unwrap_or_else(|| previous_holon.clone());
                emit_signal(Signal::HolonDeleted {
                    action_id,
                    affected_holon,
                    previous_holon,
                    timestamp,
                })?;
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
