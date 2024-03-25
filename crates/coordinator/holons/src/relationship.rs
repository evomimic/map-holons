use crate::holon_errors::HolonError;
use crate::smart_collection::SmartCollection;
use crate::staged_collection::StagedCollection;
use hdk::prelude::*;
use shared_types_holon::{HolonId, MapString};
use std::collections::BTreeMap;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct RelationshipMap(pub BTreeMap<RelationshipName, RelationshipTarget>);
impl RelationshipMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct RelationshipTarget {
    pub editable: Option<StagedCollection>, // Mutable collection
    pub cursors: Vec<SmartCollection>,      // a set of immutable, access path specific collections
}
impl RelationshipTarget {
    pub fn new() -> Self {
        Self {
            editable: None,
            cursors: Vec::new(),
        }
    }
    pub fn new_staged(editable: StagedCollection) -> Self {
        Self {
            editable: Some(editable),
            cursors: Vec::new(),
        }
    }

    // pub fn commit(&mut self, source_id: HolonId) -> Result<(), HolonError> {
    //     if let Some(collection) = self.editable.clone() {
    //         let mut mut_collection: StagedCollection = collection;
    //         mut_collection.commit(source_id)?;
    //     }
    //     Ok(())
    // }
    pub fn commit(&self, source_id: HolonId) -> Result<(), HolonError> {
        if let Some(collection) = self.editable.clone() {
            collection.commit(source_id)?;
        }
        Ok(())
    }
}

// pub fn query_relationship(
//     source_holon: HolonReference,
//     relationship_name: RelationshipName,
//     // query_spec: QuerySpec
// )
//     ->SmartCollection {
//     todo!()
// }
