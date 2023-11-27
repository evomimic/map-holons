use hdi::prelude::*;
use derive_new::new;
use std::collections::btree_map::BTreeMap;

pub type PropertyName = String;
pub type PropertyMap = BTreeMap< PropertyName, PropertyValue>;

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, new)]
pub enum PropertyValue {
    StringValue(String),
    BooleanValue(bool),
    IntegerValue(i64),
}

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}
