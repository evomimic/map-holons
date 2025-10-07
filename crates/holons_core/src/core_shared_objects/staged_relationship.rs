//! # StagedRelationshipMap
//!
//! This module provides the implementation for `StagedRelationshipMap`, a core component
//! designed to manage relationships and their associated collections of holon references.
//! The following design principles and key elements inform this implementation:
//!
//! ## Key Design Elements
//!
//! 1. **Consistency Across Maps:**
//!    - The `StagedRelationshipMap` and `RelationshipMap` structures follow the same general design approach,
//!      ensuring consistency in behavior and API usage. Both structures encapsulate a map of relationships
//!      (`RelationshipName` as keys) to `HolonCollection` objects as values, though their specific mutability
//!      and use cases differ:
//!        - `StagedRelationshipMap` represents *staged* relationships (mutable collections under construction).
//!        - `RelationshipMap` represents *saved* relationships (read-only collections already persisted).
//!
//! 2. **Encapsulation:**
//!    - The internal map (`map`) is private, with access provided only through controlled public methods like
//!      `related_holons`, `insert`, and `remove`. This ensures:
//!        - Better control over how relationships and holons are accessed or modified.
//!        - Prevention of unintended direct manipulation of the internal map.
//!
//! 3. **Interior Mutability with Controlled Immutability:**
//!    - For `StagedRelationshipMap`, each `HolonCollection` is stored as an `Rc<RefCell<HolonCollection>>`:
//!        - `Rc` provides shared ownership.
//!        - `RefCell` enables interior mutability, allowing updates to individual holon collections
//!          without requiring mutable access to the entire map.
//!    - The `related_holons` method enforces immutability at the API level by returning
//!      `Rc<HolonCollection>` instead of exposing the underlying `RefCell`.
//!
//! 4. **Serialization and Deserialization:**
//!    - The `StagedRelationshipMap` and its contents are fully serializable and deserializable
//!      using `serde`.
//!        - `HolonCollection` objects are serialized/deserialized in their entirety.
//!        - Upon deserialization, `HolonCollection` objects are wrapped in `Rc<RefCell>` to
//!          restore the original runtime mutability.
//!
//! 5. **Extensibility:**
//!    - The named-field design (`map` as a named field) allows for easy addition of new fields (e.g., metadata,
//!      timestamps, or validation rules) in the future without breaking the existing API.
//!
//! ## Intent for StagedRelationshipMap
//!
//! The `StagedRelationshipMap` is intended for use cases where relationships and their associated
//! holon collections are being actively modified or constructed. Key methods include:
//! - `related_holons`: Retrieves a holon collection for a given relationship as an immutable reference
//!   (`Rc<HolonCollection>`).
//! - `insert` and `remove`: Add or remove relationships and their associated collections.
//! - `clone_for_new_source`: Produces a deep clone of the entire map and its holon collections, resetting
//!   them for use in a new context.
//!
//! ## Shared Philosophy for RelationshipMap
//!
//! The `RelationshipMap` shares many of these design goals but is geared toward *read-only*
//! relationships (e.g., those already persisted or immutable). While `StagedRelationshipMap`
//! provides mutable access to its collections, `RelationshipMap` does not employ `RefCell`
//! because its collections are immutable.
//!
//! ## Conclusion
//!
//! By following these principles, `StagedRelationshipMap` and `RelationshipMap` provide
//! a consistent and extensible foundation for managing holon relationships, balancing the
//! need for flexibility (via interior mutability) with clear, immutable APIs where appropriate.

use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use super::{ReadableRelationship, TransientRelationshipMap, WritableRelationship};
use crate::core_shared_objects::HolonCollection;
use crate::{HolonCollectionApi, HolonReference, HolonsContextBehavior};
use core_types::{HolonError, RelationshipName};

/// Map of relationship names to `HolonCollection`s under construction.
#[derive(new, Clone, Debug)]
pub struct StagedRelationshipMap {
    pub map: BTreeMap<RelationshipName, Arc<RwLock<HolonCollection>>>,
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
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
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
            .add_references(context, holons)?;
        Ok(())
    }

    /// Removes holon references from a staged relationship, erroring if the relationship is absent.
    fn remove_related_holons(
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
                "No matching collection found in map".to_string(),
            ))
        }
    }
}

impl Serialize for StagedRelationshipMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ser_map: BTreeMap<RelationshipName, HolonCollection> = self
            .map
            .iter()
            .map(|(k, lock)| {
                let collection = lock.read().map_err(|e| {
                    serde::ser::Error::custom(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?;
                Ok((k.clone(), collection.clone()))
            })
            .collect::<Result<_, _>>()?;
        ser_map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StagedRelationshipMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tmp: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::deserialize(deserializer)?;
        let map = tmp.into_iter().map(|(k, v)| (k, Arc::new(RwLock::new(v)))).collect();
        Ok(StagedRelationshipMap::new(map))
    }
}
