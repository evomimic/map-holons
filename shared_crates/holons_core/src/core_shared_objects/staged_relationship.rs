//! # StagedRelationshipMap
//!
//! `StagedRelationshipMap` manages in-progress holon relationships in a way that is both
//! **thread-safe** and consistent with other relationship models in the MAP architecture.
//!
//! ## Key Design Principles
//!
//! 1. **Thread-Safe Interior Mutability:**
//!    - Internally, each `HolonCollection` is wrapped in an `Arc<RwLock<...>>`.
//!        - `Arc` enables safe shared ownership across threads.
//!        - `RwLock` enables concurrent read access with exclusive write access.
//!    - This replaces the prior `Rc<RefCell<>>` design, making the entire map safe for
//!      use across threads and aligning with MAP’s concurrency requirements.
//!
//! 2. **Consistent API with `RelationshipMap`:**
//!    - Like `RelationshipMap`, this structure maps `RelationshipName`s to holon collections.
//!    - Unlike `RelationshipMap`, it supports **in-place mutations** during staging workflows.
//!
//! 3. **Encapsulation and Controlled Access:**
//!    - The internal map is private and manipulated only via public trait methods.
//!    - Read and write access to `HolonCollection`s is guarded through locking, which
//!      gracefully fails with structured `HolonError::FailedToAcquireLock` errors.
//!
//! 4. **Wire Serialization Support:**
//!    - Use [`StagedRelationshipMapWire`] to serialize/deserialize staged relationships across IPC.
//!    - Runtime maps remain context-bound and are bound via [`StagedRelationshipMapWire::bind`].
//!
//! 5. **Clone for New Source:**
//!    - The `clone_for_new_source()` method produces a deep clone of the map, creating
//!      new `HolonCollection`s with reset source information—useful for cloning staged
//!      relationships into a new editing context.
//!
//! ## Intended Use
//!
//! `StagedRelationshipMap` is used when **modifying relationships in-memory prior to commit**,
//! particularly during transient or staged phases. Key use cases include:
//!
//! - Adding/removing related holons from a relationship
//! - Cloning staged holons and their relationships into a new editing context
//! - Serializing the current staged relationship state for syncing across boundaries
//!
//!  *They cannot contain TransientRefefences, since relationsips from Staged -> Transient are not allowed.
//!
//! ## Relationship to `RelationshipMap`
//!
//! - `StagedRelationshipMap`: mutable, thread-safe, used for in-progress relationships
//! - `RelationshipMap`: immutable, used for persisted or read-only relationships
//!
//! Together, they represent a consistent, phase-aware pattern for modeling holon relationships
//! across different lifecycle states within the MAP.

use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use super::{ReadableRelationship, TransientRelationshipMap, WritableRelationship};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{HolonCollection, HolonCollectionWire};
use crate::{HolonCollectionApi, HolonReference, HolonsContextBehavior};
use base_types::MapString;
use core_types::{HolonError, RelationshipName};

/// Map of relationship names to `HolonCollection`s that are under construction.
#[derive(new, Clone, Debug)]
pub struct StagedRelationshipMap {
    pub map: BTreeMap<RelationshipName, Arc<RwLock<HolonCollection>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedRelationshipMapWire {
    pub map: BTreeMap<RelationshipName, HolonCollectionWire>,
}

// Manual PartialEq implementation to compare underlying HolonCollection values in RwLocks
impl PartialEq for StagedRelationshipMap {
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

impl Eq for StagedRelationshipMap {}

impl StagedRelationshipMap {
    /// Creates a new, empty `StagedRelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// Returns an iterator over all staged relationships.
    pub fn iter(&self) -> impl Iterator<Item = (&RelationshipName, &Arc<RwLock<HolonCollection>>)> {
        self.map.iter()
    }

    /// Checks if there are no staged relationships.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn to_wire(&self) -> StagedRelationshipMapWire {
        StagedRelationshipMapWire::from(self)
    }
}

impl StagedRelationshipMapWire {
    pub fn bind(
        self,
        context: Arc<TransactionContext>,
    ) -> Result<StagedRelationshipMap, HolonError> {
        let mut map = BTreeMap::new();

        for (name, collection_wire) in self.map {
            let collection = collection_wire.bind(Arc::clone(&context))?;

            for member in collection.get_members() {
                if member.is_transient() {
                    return Err(HolonError::InvalidRelationship(
                        name.to_string(),
                        "StagedRelationshipMap cannot contain TransientReferences".to_string(),
                    ));
                }
            }

            map.insert(name, Arc::new(RwLock::new(collection)));
        }

        Ok(StagedRelationshipMap::new(map))
    }
}

impl From<&StagedRelationshipMap> for StagedRelationshipMapWire {
    fn from(map: &StagedRelationshipMap) -> Self {
        let mut wire_map = BTreeMap::new();

        for (name, lock) in map.map.iter() {
            let collection = lock.read().expect("Failed to acquire read lock on holon collection");
            wire_map.insert(name.clone(), HolonCollectionWire::from(&*collection));
        }

        Self { map: wire_map }
    }
}

impl ReadableRelationship for StagedRelationshipMap {
    /// Produces a new TransientRelationshipMap by cloning each holon collection for a new source.
    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
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

    /// Retrieves a holon collection for the given relationship, creating an empty one if absent.
    fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Arc<RwLock<HolonCollection>> {
        self.map
            .get(relationship_name)
            .cloned()
            .unwrap_or_else(|| Arc::new(RwLock::new(HolonCollection::new_staged())))
    }
}

impl WritableRelationship for StagedRelationshipMap {
    /// Adds holon references to a staged relationship, creating the collection if needed.
    fn add_related_holons(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        // TODO: Add check that holons to be added can only be StagedReferences
        let lock = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Arc::new(RwLock::new(HolonCollection::new_staged())));
        lock.write()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on holon collection: {}",
                    e
                ))
            })?
            .add_references(holons)?;
        Ok(())
    }

    /// Adds holon references with precomputed keys, avoiding key lookups during mutation.
    fn add_related_holons_with_keys(
        &mut self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError> {
        let lock = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Arc::new(RwLock::new(HolonCollection::new_staged())));
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

    /// Removes holon references from a staged relationship, erroring if the relationship is absent.
    fn remove_related_holons(
        &mut self,
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
                .remove_references(holons)?;
            Ok(())
        } else {
            Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", relationship_name),
                "No matching collection found in map".to_string(),
            ))
        }
    }

    /// Removes holon references with precomputed keys, avoiding key lookups during mutation.
    fn remove_related_holons_with_keys(
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
                "No matching collection found in map".to_string(),
            ))
        }
    }
}
