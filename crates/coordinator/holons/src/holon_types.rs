use derive_new::new;
use std::fmt;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyMap;



#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct Holon {
    pub state: HolonState,
    pub saved_node: Option<Record>, // The last saved state of HolonNode. None = not yet created
    pub property_map: PropertyMap,
    // pub descriptor: HolonReference,
    // pub holon_space: HolonReference,
    // pub outbound_relationships: RelationshipMap,
    // pub dances : DanceMap,
}
// impl fmt::Display for Holon {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "Holon: has state: {0}", self.state)
//
//     }
// }

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum HolonState {
    New,
    Fetched,
    Changed,
    // CreateInProgress,
    // SaveInProgress,
}
impl fmt::Display for HolonState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HolonState::New => write!(f, "New"),
            HolonState::Fetched => write!(f, "Fetched"),
            HolonState::Changed => write!(f, "Changed"),
        }
    }
}
