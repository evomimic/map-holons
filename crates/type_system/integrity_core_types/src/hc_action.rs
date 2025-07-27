use derive_new::new;
use serde::{Deserialize, Serialize};

use crate::{
    LocalId, PersistenceAgentId, PersistenceLinkTag, PersistenceLinkType, PersistenceTimestamp,
};

/// Holochain-independent model for a DHT Action.
///
/// This type is used for shared validation and application logic,
/// and intentionally avoids any dependency on Holochain types.
///
/// It is the responsibility of Holochain guest code to convert between
/// this object and the Holochain-annotated `Action` enum.
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersistenceAction {
    // Dna(Dna),
    // AgentValidationPkg(AgentValidationPkg),
    // InitZomesComplete(InitZomesComplete),
    CreateLink(PersistenceCreateLink),
    DeleteLink(PersistenceDeleteLink),
    // CloseChain(CloseChain),
    // OpenChain(OpenChain),
    Create(PersistenceCreate),
    Update(PersistenceUpdate),
    Delete,
}

#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub struct HCCreate<W = RateWeight> {
pub struct PersistenceCreate {
    pub author: PersistenceAgentId,
    pub timestamp: PersistenceTimestamp,
    pub action_seq: u32,
    pub prev_action: LocalId,
    // pub entry_type: EntryType,
    // pub entry_hash: EntryHash,

    // pub weight: W,
}

#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub struct PersistenceUpdate<W = RateWeight> {
pub struct PersistenceUpdate {
    pub author: PersistenceAgentId,
    pub timestamp: PersistenceTimestamp,
    pub action_seq: u32,
    pub prev_action: LocalId,
    // pub original_action_address: ActionHash,
    // pub original_entry_address: EntryHash,

    // pub entry_type: EntryType,
    // pub entry_hash: EntryHash,

    // pub weight: W,
}

#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub struct PersistenceDelete<W = RateWeight> {
pub struct PersistenceDelete {
    pub author: PersistenceAgentId,
    pub timestamp: PersistenceTimestamp,
    pub action_seq: u32,
    pub prev_action: LocalId,

    // pub deletes_address: HoloHash<Action>,
    // pub deletes_entry_address: HoloHash<Entry>,

    // pub weight: W,
}


#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistenceCreateLink {
    // pub struct CreateLink<W = RateWeight> {
    pub author: PersistenceAgentId,
    pub timestamp: PersistenceTimestamp,
    pub action_seq: u32,
    pub prev_action: LocalId,

    pub base_address: LocalId,
    pub target_address: LocalId,
    //     pub zome_index: ZomeIndex,
    pub link_type: PersistenceLinkType,
    pub tag: PersistenceLinkTag,

    //     pub weight: W,
}

#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistenceDeleteLink {
    pub author: PersistenceAgentId,
    pub timestamp: PersistenceTimestamp,
    pub action_seq: u32,
    pub prev_action: LocalId,

    pub base_address: LocalId,
    // pub link_add_address: HoloHash<Action>,  // What is this?
}
