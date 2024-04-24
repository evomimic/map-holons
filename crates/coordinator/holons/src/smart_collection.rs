use hdk::prelude::*;
use std::collections::{BTreeMap};
use derive_new::new;
use shared_types_holon::MapString;
use crate::holon_reference::HolonReference;
use crate::smart_reference::SmartReference;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartCollection {
    pub source_holon: Option<HolonReference>,
    pub relationship_descriptor: Option<HolonReference>,
    pub access_path: Option<HolonReference>,
    pub holons: Vec<SmartReference>,
    pub keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
    // query_spec: QueryExpression,
    //

}

