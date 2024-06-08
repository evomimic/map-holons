use crate::{holon_error::HolonError, staged_reference::StagedReference};
// use crate::smart_reference::SmartReference;
use crate::context::HolonsContext;
use crate::holon_reference::HolonReference;
use crate::smart_collection::SmartCollection;
use crate::staged_collection::StagedCollection;
use hdk::prelude::*;
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


    /// This method saves outbound relationships from the holon identified by source_id
    /// to the holons that are in the editable StagedCollection of this relationship.
    pub fn commit_relationship(
        &self,
        context: &HolonsContext,
        source_id: HolonId,
        name: RelationshipName,
    ) -> Result<(), HolonError> {
        if let Some(collection) = self.editable.clone() {
            collection.add_smartlinks_for_collection(context, source_id.clone(), name.clone())?;
        }
        Ok(())
    }

    /// Creates an editable_collection within the RelationshipTarget from the SmartReferences in the existing_collection
    pub fn stage_collection(
        &mut self,
        source_holon: StagedReference,
        existing_collection: SmartCollection,
    ) {
        // convert Vec<SmartReference> to Vec<HolonReference>
        let holons = existing_collection
            .holons
            .into_iter()
            .map(|smart_ref| HolonReference::Smart(smart_ref))
            .collect();

        let staged_collection = StagedCollection {
            source_holon: Some(source_holon),
            relationship_descriptor: existing_collection.relationship_descriptor,
            holons,
            keyed_index: existing_collection.keyed_index,
        };
        self.editable = Some(staged_collection);
    }
}

// impl Clone for RelationshipTarget {
//     /// Custom clone implementation, does not clone its cursors or editable vector
//     fn clone(&self) -> Self {
//         Self {
//             editable: None,
//             cursors: Vec::new(),
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
