use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use hdk::prelude::*;
use derive_new::new;
use serde::{Deserialize, Serialize};

use crate::{CollectionState, HolonCollectionApi, HolonReference, HolonsContextBehavior, StagedRelationshipMap};

use super::{
    HolonCollection, HolonError, ReadableRelationship, RelationshipName, WritableRelationship,
};

/// Represents a map of transient relationships, where the keys are relationship names and the values
/// are fully-loaded collections of holons for those relationships. Absence of an entry indicates
/// that the relationship has no associated holons.
#[derive(new, SerializedBytes, Clone, Debug, Eq, PartialEq)]
pub struct TransientRelationshipMap {
    pub map: BTreeMap<RelationshipName, Rc<RefCell<HolonCollection>>>,
}

impl TransientRelationshipMap {
    /// Creates a new, empty `TransientRelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// Returns an iterator over the key-value pairs in the map. This is primarily intended for
    /// use by adapters that serialize TransientRelationshipMap into other representations
    /// (e.g., json adapter).
    pub fn iter(&self) -> impl Iterator<Item = (&RelationshipName, &Rc<RefCell<HolonCollection>>)> {
        self.map.iter()
    }

    /// Returns `true` if the map contains no relationships.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Converts to a StagedRelationshipMap
    pub fn to_staged(self) -> Result<StagedRelationshipMap, HolonError> { 
        for (_name, collection) in self.map.iter() {
            let mut staged_collection = collection.borrow_mut();
            staged_collection.mark_as_staged()?;
        }
        let staged = StagedRelationshipMap::new(self.map);

        Ok(staged)
    }
}

impl ReadableRelationship for TransientRelationshipMap {
    // =====================
    //     CONSTRUCTORS
    // =====================

    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
        let mut cloned_relationship_map = BTreeMap::new();

        for (name, collection) in &self.map {
            let cloned_collection = collection.borrow().clone_for_new_source()?; // Assumes `clone_for_new_source` exists on `HolonCollection`.
            cloned_relationship_map.insert(name.clone(), Rc::new(RefCell::new(cloned_collection)));
        }

        Ok(TransientRelationshipMap::new(cloned_relationship_map))
    }

    // ====================
    //    DATA ACCESSORS
    // ====================

    fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection> {
        if let Some(rc_refcell) = self.map.get(relationship_name) {
            // Borrow the RefCell and clone the inner HolonCollection
            Rc::new(rc_refcell.borrow().clone())
        } else {
            // Return a new Rc<HolonCollection> if the entry doesn't exist
            Rc::new(HolonCollection::new_staged())
        }
    }
}

impl WritableRelationship for TransientRelationshipMap {
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        // Retrieve or create the collection for the specified relationship name
        let collection = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Rc::new(RefCell::new(HolonCollection::new_staged())));

        // Borrow the `HolonCollection` mutably to add the supplied holons
        collection.borrow_mut().add_references(context, holons)?;

        Ok(())
    }

    fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        if let Some(collection) = self.map.get(relationship_name) {
            // Borrow the `HolonCollection` mutably to remove the supplied holons
            collection.borrow_mut().remove_references(context, holons)?;
            Ok(())
        } else {
            Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", relationship_name),
                "No matching collection found in the map.".to_string(),
            ))
        }
    }
}

impl Serialize for TransientRelationshipMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Create a serializable version of the map by cloning the inner `HolonCollection`
        let serializable_map: BTreeMap<_, _> = self
            .map
            .iter()
            .map(|(key, value)| (key.clone(), value.borrow().clone())) // Clone the inner `HolonCollection`
            .collect();

        serializable_map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TransientRelationshipMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize into a temporary BTreeMap<RelationshipName, HolonCollection>
        let deserialized_map: BTreeMap<RelationshipName, HolonCollection> =
            BTreeMap::deserialize(deserializer)?;

        // Wrap each value in Rc<RefCell>
        let wrapped_map: BTreeMap<_, _> = deserialized_map
            .into_iter()
            .map(|(key, value)| (key, Rc::new(RefCell::new(value))))
            .collect();

        Ok(Self { map: wrapped_map })
    }
}
