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

#[derive(Clone, PartialEq, Eq)]
pub struct HolonId(pub ActionHash);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct PropertyName(pub MapString);
