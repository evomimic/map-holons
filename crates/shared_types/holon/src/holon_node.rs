use hdi::prelude::*;
use derive_new::new;
use std::collections::btree_map::BTreeMap;
use crate::value_types::{BaseValue, MapString};

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}
pub type HolonId = ActionHash;
pub type PropertyName = MapString;
pub type PropertyMap = BTreeMap<PropertyName, BaseValue>;


