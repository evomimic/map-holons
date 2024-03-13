use crate::value_types::{BaseValue, MapString};
use derive_new::new;
use hdi::prelude::*;
use std::collections::btree_map::BTreeMap;
use std::fmt;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}
pub type PropertyValue = BaseValue;
pub type PropertyMap = BTreeMap<PropertyName, PropertyValue>;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct HolonId(pub ActionHash);
impl From<ActionHash> for HolonId {
    fn from(action_hash: ActionHash) -> Self {
        HolonId(action_hash)
    }
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyName(pub MapString);
impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegate formatting to the inner MapString
        write!(f, "{}", self.0)
    }
}