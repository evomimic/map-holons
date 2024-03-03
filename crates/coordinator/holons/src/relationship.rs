use hdk::prelude::*;
use shared_types_holon::value_types::MapString;
use std::collections::BTreeMap;
use derive_new::new;
use crate::staged_collection::StagedCollection;

#[hdk_entry_helper]
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct RelationshipName(pub MapString);

#[hdk_entry_helper]
#[derive(new, Clone )]
pub enum RelationshipTarget{
    Staged(StagedCollection), // Mutable collection
    //ReadOnly(SmartCollection), // Immutable collection
}

pub type RelationshipMap = BTreeMap<RelationshipName, RelationshipTarget>;
