use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

use super::{ReadableRelationship, TransientRelationshipMap};
use crate::{core_shared_objects::HolonCollection, StagedRelationshipMap};
use core_types::{HolonError, RelationshipName};

/// Custom RelationshipMap is only used for caching and will never be serialized
#[derive(new, Clone, Debug)]
pub struct RelationshipMap {
    map: HashMap<RelationshipName, Arc<RwLock<HolonCollection>>>,
}
impl RelationshipMap {
    /// Creates a new, empty `RelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: HashMap::new() }
    }

    /// Converts to a StagedRelationshipMap.
    pub fn clone_for_staged(&self) -> Result<StagedRelationshipMap, HolonError> {
        let mut cloned_map = BTreeMap::new();
        for (name, arc_lock) in self.map.iter() {
            let collection = arc_lock
                .read()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?
                .clone_for_staged()?;
            cloned_map.insert(name.clone(), Arc::new(RwLock::new(collection)));
        }
        Ok(StagedRelationshipMap::new(cloned_map))
    }

    /// Returns a shared reference (`Arc<RwLock<HolonCollection>>`) for the given `relationship_name`.
    /// Returns `None` if the relationship is not found.
    pub fn get_collection_for_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Option<Arc<RwLock<HolonCollection>>> {
        self.map.get(relationship_name).cloned()
    }
    /// Inserts a `HolonCollection` into the `RelationshipMap` for the given `relationship_name`.
    pub fn insert(
        &mut self,
        relationship_name: RelationshipName,
        collection: Arc<RwLock<HolonCollection>>,
    ) {
        self.map.insert(relationship_name, collection);
    }

    /// Iterates over all relationships in the `RelationshipMap`.
    /// Returns a vector of `(RelationshipName, Arc<RwLock<HolonCollection>>)` pairs for read-only access.
    pub fn iter(&self) -> Vec<(RelationshipName, Arc<RwLock<HolonCollection>>)> {
        self.map.iter().map(|(k, v)| (k.clone(), Arc::clone(v))).collect()
    }
}

impl ReadableRelationship for RelationshipMap {
    // =====================
    //     CONSTRUCTORS
    // =====================

    /// Since all Holons begin their lifecylce as Transient, so too does their relationship_map.
    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
        let mut cloned_map = BTreeMap::new();
        for (name, arc_lock) in self.map.iter() {
            let cloned_collection = arc_lock
                .read()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?
                .clone_for_new_source()?;
            cloned_map.insert(name.clone(), Arc::new(RwLock::new(cloned_collection)));
        }
        Ok(TransientRelationshipMap::new(cloned_map))
    }

    // ====================
    //    DATA ACCESSORS
    // ====================

    fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Arc<RwLock<HolonCollection>> {
        if let Some(arc_lock) = self.map.get(relationship_name) {
            Arc::clone(arc_lock)
        } else {
            Arc::new(RwLock::new(HolonCollection::new_staged()))
        }
    }
}

// Implement Serialize for RelationshipMap
impl Serialize for RelationshipMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializable_map = HashMap::new();
        for (key, arc_lock) in &self.map {
            let coll = arc_lock.read().map_err(|e| {
                serde::ser::Error::custom(format!(
                    "Failed to acquire read lock on holon collection: {}",
                    e
                ))
            })?;
            serializable_map.insert(key.clone(), coll.clone());
        }
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
        let wrapped_map: HashMap<_, _> = deserialized_map
            .into_iter()
            .map(|(key, value)| (key, Arc::new(RwLock::new(value))))
            .collect();

        Ok(RelationshipMap { map: wrapped_map })
    }
}

impl From<StagedRelationshipMap> for RelationshipMap {
    fn from(staged: StagedRelationshipMap) -> Self {
        let mut new_map = HashMap::new();

        for (name, arc_lock) in staged.map {
            let cloned_collection =
                arc_lock.read().expect("Failed to acquire read lock on holon collection").clone();
            new_map.insert(name, Arc::new(RwLock::new(cloned_collection)));
        }

        RelationshipMap::new(new_map)
    }
}

impl From<TransientRelationshipMap> for RelationshipMap {
    fn from(transient: TransientRelationshipMap) -> Self {
        let mut new_map = HashMap::new();

        for (name, arc_lock) in transient.map {
            let cloned_collection =
                arc_lock.read().expect("Failed to acquire read lock on holon collection").clone();
            new_map.insert(name, Arc::new(RwLock::new(cloned_collection)));
        }

        RelationshipMap::new(new_map)
    }
}
