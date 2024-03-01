use hdk::prelude::*;
use shared_types_holon::value_types::MapString;
use std::collections::BTreeMap;
use derive_new::new;
use crate::relationship::RelationshipName;

use crate::staged_reference::StagedReference;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum RelationshipCollection {
    Staged(StagedCollection), // Mutable collection
    //ReadOnly(SmartCollection), // Immutable collection
}
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedCollection {
    holon_map: BTreeMap<RelationshipName,RelationshipReference>,

}
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum RelationshipReference {
    Staged(StagedReference), // a reference to a Staged Holon
    //Existing(SmartReference), //  a reference to an existing Holon
}
// #[hdk_entry_helper]
// #[derive(new, Clone, PartialEq, Eq)]
// pub struct RelationshipName(pub MapString);

// pub type RelationshipMap = BTreeMap<RelationshipName, RelationshipCollection>;
