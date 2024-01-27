use crate::value_types::{BaseValue, MapString};
use derive_new::new;
use hdi::prelude::*;
use std::collections::btree_map::BTreeMap;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}

pub type PropertyMap = BTreeMap<PropertyName, BaseValue>;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct HolonId(pub ActionHash);
impl From<ActionHash> for HolonId {
    fn from(action_hash: ActionHash) -> Self {
        HolonId(action_hash)
    }
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct PropertyName(pub MapString);
