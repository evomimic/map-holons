#![allow(unused_imports)]

use crate::shared_objects_layer::{HolonCollection, HolonError};
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

    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        let mut cloned_relationship_map = BTreeMap::new();

        for (name, collection) in self.0.clone() {
            let cloned_collection = collection.clone_for_new_source()?;
            cloned_relationship_map.insert(name, cloned_collection);
        }

        Ok(RelationshipMap(cloned_relationship_map))
    }

    pub fn get_collection_for_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Option<&HolonCollection> {
        self.0.get(&relationship_name)
    }
}

// #[derive(Clone, Serialize, Deserialize, Debug)]
// pub struct SmartLinkHolder {
//     pub name: RelationshipName,
//     pub reference: HolonReference,
// }
