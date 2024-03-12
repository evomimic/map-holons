use hdk::prelude::*;
use std::collections::{BTreeMap};
use derive_new::new;
use shared_types_holon::MapString;
use crate::holon_reference::HolonReference;
use crate::smart_reference::SmartReference;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartCollection {
    source_holon: Option<HolonReference>,
    relationship_descriptor: Option<HolonReference>,
    access_path: Option<HolonReference>,
    holons: Vec<SmartReference>,
    keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
    // query_spec: QueryExpression,
    //

}

