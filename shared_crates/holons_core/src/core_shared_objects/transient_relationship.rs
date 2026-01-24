use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use derive_new::new;
use serde::{Deserialize, Serialize};

use super::{HolonCollection, HolonCollectionWire, ReadableRelationship, WritableRelationship};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonCollectionApi, HolonReference, HolonsContextBehavior, StagedRelationshipMap};
use base_types::MapString;
use core_types::{HolonError, RelationshipName};

/// Represents a map of transient relationships, where the keys are relationship names and the values
/// are fully-loaded collections of holons for those relationships. Absence of an entry indicates
/// that the relationship has no associated holons.
#[derive(new, Debug, Clone)]
pub struct TransientRelationshipMap {
    pub map: BTreeMap<RelationshipName, Arc<RwLock<HolonCollection>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientRelationshipMapWire {
    pub map: BTreeMap<RelationshipName, HolonCollectionWire>,
}

// Manual PartialEq implementation to compare underlying HolonCollection values in RwLocks
impl PartialEq for TransientRelationshipMap {
    fn eq(&self, other: &Self) -> bool {
        if self.map.len() != other.map.len() {
            return false;
        }
        for (key, lock) in &self.map {
            let other_lock = match other.map.get(key) {
                Some(l) => l,
                None => return false,
            };
            let this_coll = lock.read().expect("Failed to acquire read lock on holon collection");
            let other_coll =
                other_lock.read().expect("Failed to acquire read lock on holon collection");
            if *this_coll != *other_coll {
                return false;
            }
        }
        true
    }
}

impl Eq for TransientRelationshipMap {}

impl TransientRelationshipMap {
    /// Creates a new, empty `TransientRelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// Clones the `TransientRelationshipMap` for a new source. The `HolonCollection` objects are also cloned
    /// for the new source using their `clone_for_new_source` method.
    ///
    /// # Returns
    /// - `Ok(Self)`: A new `TransientRelationshipMap` with cloned `HolonCollection` objects.
    /// - `Err(HolonError)`: If cloning any `HolonCollection` fails.
    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        let mut new_map = BTreeMap::new();
        for (name, lock) in &self.map {
            let coll = lock
                .read()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?
                .clone_for_new_source()?;
            new_map.insert(name.clone(), Arc::new(RwLock::new(coll)));
        }
        Ok(TransientRelationshipMap::new(new_map))
    }

    /// Adds the specified holons to the collection associated with the given relationship name.
    /// If a collection for the relationship already exists, the holons are added to it.
    /// If no such collection exists, a new one is created and inserted into the map.
    ///
    /// # Arguments
    /// - `relationship_name`: The name of the relationship to modify or create.
    /// - `context`: The operational context for validation and access.
    /// - `holons`: A list of `HolonReference`s to add to the collection.
    ///
    /// # Errors
    /// - Returns an error if adding references fails due to validation or other issues.
    pub fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        // Retrieve or create the collection for the specified relationship name
        let lock = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Arc::new(RwLock::new(HolonCollection::new_transient())));
        lock.write()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on holon collection: {}",
                    e
                ))
            })?
            .add_references(context, holons)?;
        Ok(())
    }

    /// Adds holons using precomputed keys to avoid key lookups during mutation.
    pub fn add_related_holons_with_keys(
        &mut self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError> {
        let lock = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Arc::new(RwLock::new(HolonCollection::new_transient())));
        lock.write()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on holon collection: {}",
                    e
                ))
            })?
            .add_references_with_keys(entries)?;
        Ok(())
    }

    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Arc<RwLock<HolonCollection>>`.
    ///
    /// If the `relationship_name` exists in the `TransientRelationshipMap`, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Arc<RwLock<HolonCollection>>`.
    ///
    /// If the `relationship_name` exists in the `TransientRelationshipMap`, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    pub fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Arc<RwLock<HolonCollection>> {
        self.map
            .get(relationship_name)
            .cloned()
            .unwrap_or_else(|| Arc::new(RwLock::new(HolonCollection::new_transient())))
    }

    /// Removes the specified holons from the collection associated with the given relationship name.
    ///
    /// If the relationship exists, the supplied holons are removed from its collection.
    /// If the relationship doesn't exist, an error is returned.
    ///
    /// # Arguments
    /// - `relationship_name`: The name of the relationship to modify.
    /// - `context`: The operational context for validation and access.
    /// - `holons`: A list of `HolonReference`s to remove from the collection.
    ///
    /// # Errors
    /// - Returns an error if the relationship doesn't exist.
    /// - Returns an error if removing references fails due to validation or other issues.
    pub fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        if let Some(lock) = self.map.get(relationship_name) {
            lock.write()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire write lock on holon collection: {}",
                        e
                    ))
                })?
                .remove_references(context, holons)?;
            Ok(())
        } else {
            Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", relationship_name),
                "No matching collection found in the map.".to_string(),
            ))
        }
    }

    /// Removes holons using precomputed keys to avoid key lookups during mutation.
    pub fn remove_related_holons_with_keys(
        &mut self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError> {
        if let Some(lock) = self.map.get(relationship_name) {
            lock.write()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire write lock on holon collection: {}",
                        e
                    ))
                })?
                .remove_references_with_keys(entries)?;
            Ok(())
        } else {
            Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", relationship_name),
                "No matching collection found in the map.".to_string(),
            ))
        }
    }

    /// Returns an iterator over the key-value pairs in the map. This is primarily intended for
    /// use by adapters that serialize TransientRelationshipMap into other representations
    /// (e.g., json adapter).
    pub fn iter(&self) -> impl Iterator<Item = (&RelationshipName, &Arc<RwLock<HolonCollection>>)> {
        self.map.iter()
    }

    /// Returns `true` if the map contains no relationships.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Converts to a StagedRelationshipMap
    pub fn to_staged(self) -> Result<StagedRelationshipMap, HolonError> {
        for (_name, collection) in self.map.iter() {
            let mut staged_collection = collection.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on holon collection: {}",
                    e
                ))
            })?;
            staged_collection.mark_as_staged()?;
        }
        let staged = StagedRelationshipMap::new(self.map);

        Ok(staged)
    }

    pub fn to_wire(&self) -> TransientRelationshipMapWire {
        TransientRelationshipMapWire::from(self)
    }
}

impl TransientRelationshipMapWire {
    pub fn bind(
        self,
        context: Arc<TransactionContext>,
    ) -> Result<TransientRelationshipMap, HolonError> {
        let mut map = BTreeMap::new();

        for (name, collection_wire) in self.map {
            let collection = collection_wire.bind(Arc::clone(&context))?;
            map.insert(name, Arc::new(RwLock::new(collection)));
        }

        Ok(TransientRelationshipMap::new(map))
    }
}

impl From<&TransientRelationshipMap> for TransientRelationshipMapWire {
    fn from(map: &TransientRelationshipMap) -> Self {
        let mut wire_map = BTreeMap::new();

        for (name, lock) in map.map.iter() {
            let collection = lock
                .read()
                .expect("Failed to acquire read lock on holon collection");
            wire_map.insert(name.clone(), HolonCollectionWire::from(&*collection));
        }

        Self { map: wire_map }
    }
}

// Implement thread-safe trait versions
impl ReadableRelationship for TransientRelationshipMap {
    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
        TransientRelationshipMap::clone_for_new_source(self)
    }

    fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Arc<RwLock<HolonCollection>> {
        TransientRelationshipMap::get_related_holons(self, relationship_name)
    }
}

impl WritableRelationship for TransientRelationshipMap {
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        TransientRelationshipMap::add_related_holons(self, context, relationship_name, holons)
    }

    fn add_related_holons_with_keys(
        &mut self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError> {
        TransientRelationshipMap::add_related_holons_with_keys(self, relationship_name, entries)
    }

    fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        TransientRelationshipMap::remove_related_holons(self, context, relationship_name, holons)
    }

    fn remove_related_holons_with_keys(
        &mut self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError> {
        TransientRelationshipMap::remove_related_holons_with_keys(self, relationship_name, entries)
    }
}
