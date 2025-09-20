use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use super::{ReadableRelationship, TransientRelationshipMap};
use crate::{core_shared_objects::HolonCollection, StagedRelationshipMap};
use core_types::{HolonError, RelationshipName};

/// Custom RelationshipMap is only used for caching and will never be serialized
#[derive(new, Clone, Debug, Eq, PartialEq)]
pub struct RelationshipMap {
    map: HashMap<RelationshipName, Rc<HolonCollection>>,
}
impl RelationshipMap {
    /// Creates a new, empty `RelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: HashMap::new() }
    }

    /// Converts to a StagedRelationshipMap.
    pub fn clone_for_staged(&self) -> Result<StagedRelationshipMap, HolonError> {
        let mut cloned_map = BTreeMap::new();

        for (name, rc_collection) in self.map.iter() {
            let cloned_collection = rc_collection.clone_for_staged()?;
            cloned_map.insert(name.clone(), Rc::new(RefCell::new(cloned_collection.clone())));
        }

        Ok(StagedRelationshipMap::new(cloned_map))
    }

    /// Returns a shared reference (`Rc<HolonCollection>`) for the given `relationship_name`.
    /// Returns `None` if the relationship is not found.
    pub fn get_collection_for_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Option<Rc<HolonCollection>> {
        // Borrow the map immutably and clone the Rc for the requested relationship
        self.map.get(relationship_name).cloned()
    }
    /// Inserts a `HolonCollection` into the `RelationshipMap` for the given `relationship_name`.
    pub fn insert(&mut self, relationship_name: RelationshipName, collection: Rc<HolonCollection>) {
        // Borrow the map mutably and insert the new collection
        self.map.insert(relationship_name, collection);
    }

    /// Iterates over all relationships in the `RelationshipMap`.
    /// Returns a vector of `(RelationshipName, Rc<HolonCollection>)` pairs for read-only access.
    pub fn iter(&self) -> Vec<(RelationshipName, Rc<HolonCollection>)> {
        self.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

impl ReadableRelationship for RelationshipMap {
    // =====================
    //     CONSTRUCTORS
    // =====================

    /// Since all Holons begin their lifecylce as Transient, so too does their relationship_map.
    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
        let mut cloned_map = BTreeMap::new();

        for (name, rc_collection) in self.map.iter() {
            // Sets CollectionState::Transient
            let cloned_collection = rc_collection.clone_for_new_source()?; // Assumes `clone_for_new_source` exists on `HolonCollection`.
            cloned_map.insert(name.clone(), Rc::new(RefCell::new(cloned_collection)));
        }

        Ok(TransientRelationshipMap::new(cloned_map))
    }

    // ====================
    //    DATA ACCESSORS
    // ====================

    fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection> {
        if let Some(rc_collection) = self.map.get(relationship_name) {
            // Clone the inner HolonCollection
            Rc::clone(rc_collection)
        } else {
            // Return a new Rc<HolonCollection> if the entry doesn't exist
            Rc::new(HolonCollection::new_staged())
        }
    }
}

// Implement Serialize for RelationshipMap
impl Serialize for RelationshipMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let serializable_map: HashMap<_, _> = self
            .map
            .iter()
            .map(|(key, value)| (key.clone(), &**value)) // Deref Rc
            .collect();
        serializable_map.serialize(serializer)
    }
}

// Implement Deserialize for RelationshipMap
impl<'de> Deserialize<'de> for RelationshipMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let deserialized_map: HashMap<RelationshipName, HolonCollection> =
            HashMap::deserialize(deserializer)?;
        let wrapped_map: HashMap<_, _> =
            deserialized_map.into_iter().map(|(key, value)| (key, Rc::new(value))).collect();

        Ok(RelationshipMap { map: wrapped_map })
    }
}

impl From<StagedRelationshipMap> for RelationshipMap {
    fn from(staged: StagedRelationshipMap) -> Self {
        let mut new_map = HashMap::new();

        for (name, rc_refcell_collection) in staged.map {
            let cloned_collection = rc_refcell_collection.borrow().clone();
            new_map.insert(name, Rc::new(cloned_collection));
        }

        RelationshipMap::new(new_map)
    }
}

impl From<TransientRelationshipMap> for RelationshipMap {
    fn from(transient: TransientRelationshipMap) -> Self {
        let mut new_map = HashMap::new();

        for (name, rc_refcell_collection) in transient.map {
            let cloned_collection = rc_refcell_collection.borrow().clone();
            new_map.insert(name, Rc::new(cloned_collection));
        }

        RelationshipMap::new(new_map)
    }
}
