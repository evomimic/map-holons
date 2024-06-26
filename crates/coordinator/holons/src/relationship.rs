#![allow(unused_imports)]

use crate::context::HolonsContext;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::smart_reference::SmartReference;
// use crate::smart_reference::SmartReference;
use crate::holon_reference::HolonReference;
use hdk::prelude::*;
use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, MapString};
use std::collections::BTreeMap;
use std::fmt;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);
impl fmt::Display for RelationshipName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct RelationshipMap(pub BTreeMap<RelationshipName, HolonCollection>);
impl RelationshipMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SmartLinkHolder {
    pub name: RelationshipName,
    pub reference: HolonReference,
}

// impl Clone for HolonCollection {
//     /// Custom clone implementation, does not clone its cursors or editable vector
//     fn clone(&self) -> Self {
//         Self {
//             editable: None,
//             cursors: None,
//         }
//     }
// }

// pub fn query_relationship(
//     source_holon: HolonReference,
//     relationship_name: RelationshipName,
//     // query_spec: QuerySpec
// )
//     ->SmartCollection {
//     todo!()
// }
