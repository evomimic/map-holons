use hdk::prelude::*;
use shared_types_holon::value_types::MapString;
use std::collections::BTreeMap;
use crate::holon_reference::HolonReference;
use crate::smart_collection::SmartCollection;
use crate::staged_collection::StagedCollection;

#[hdk_entry_helper]
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct RelationshipName(pub MapString);

pub type RelationshipMap = BTreeMap<RelationshipName, RelationshipTarget>;



#[hdk_entry_helper]
#[derive(Clone )]
pub struct RelationshipTarget {
    pub editable: Option<StagedCollection>, // Mutable collection
    pub cursors: Vec<SmartCollection>, // a set of immutable, access path specific collections
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
            editable : Some(editable),
            cursors: Vec::new(),
        }
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

