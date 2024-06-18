use crate::holon_reference::HolonReference;
use crate::relationship::RelationshipName;
use crate::smart_reference::SmartReference;
use derive_new::new;
use hdk::prelude::*;
use shared_types_holon::MapString;
use std::collections::BTreeMap;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartCollection {
    pub source_holon: Option<HolonReference>,
    pub relationship_name: Option<RelationshipName>,
    pub holons: Vec<SmartReference>,
    // pub keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
    // query_spec: QueryExpression,
    //
}
