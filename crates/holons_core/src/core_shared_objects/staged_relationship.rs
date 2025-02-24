//! # StagedRelationshipMap and Related Design Elements
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
//!      `get_related_holons`, `insert`, and `remove`. This ensures:
//!        - Better control over how relationships and holons are accessed or modified.
//!        - Prevention of unintended direct manipulation of the internal map.
//!
//! 3. **Interior Mutability with Controlled Immutability:**
//!    - For `StagedRelationshipMap`, each `HolonCollection` is stored as an `Rc<RefCell<HolonCollection>>`:
//!        - `Rc` provides shared ownership.
//!        - `RefCell` enables interior mutability, allowing updates to individual holon collections
//!          without requiring mutable access to the entire map.
//!    - The `get_related_holons` method enforces immutability at the API level by returning
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
//! - `get_related_holons`: Retrieves a holon collection for a given relationship as an immutable reference
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
use crate::core_shared_objects::{HolonCollection, HolonError, RelationshipName};
use crate::{HolonCollectionApi, HolonReference, HolonsContextBehavior};
use hdk::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Represents a map of staged relationships, where the keys are relationship names and the values
/// are fully-loaded collections of holons for those relationships. Absence of an entry indicates
/// that the relationship has no associated holons.
#[derive(SerializedBytes, Clone, Debug, Eq, PartialEq)]
pub struct StagedRelationshipMap {
    pub map: BTreeMap<RelationshipName, Rc<RefCell<HolonCollection>>>,
}

impl StagedRelationshipMap {
    /// Creates a new, empty `StagedRelationshipMap`.
    pub fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// Clones the `StagedRelationshipMap` for a new source. The `HolonCollection` objects are also cloned
    /// for the new source using their `clone_for_new_source` method.
    ///
    /// # Returns
    /// - `Ok(Self)`: A new `StagedRelationshipMap` with cloned `HolonCollection` objects.
    /// - `Err(HolonError)`: If cloning any `HolonCollection` fails.
    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        let mut cloned_relationship_map = BTreeMap::new();

        for (name, collection) in &self.map {
            let cloned_collection = collection.borrow().clone_for_new_source()?; // Assumes `clone_for_new_source` exists on `HolonCollection`.
            cloned_relationship_map.insert(name.clone(), Rc::new(RefCell::new(cloned_collection)));
        }

        Ok(Self { map: cloned_relationship_map })
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
        let collection = self
            .map
            .entry(relationship_name)
            .or_insert_with(|| Rc::new(RefCell::new(HolonCollection::new_staged())));

        // Borrow the `HolonCollection` mutably to add the supplied holons
        collection.borrow_mut().add_references(context, holons)?;

        Ok(())
    }
    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Rc`.
    ///
    /// If the `relationship_name` exists in the `StagedRelationshipMap`, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Rc`.
    ///
    /// If the `relationship_name` exists in the `StagedRelationshipMap`, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    pub fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection> {
        if let Some(rc_refcell) = self.map.get(relationship_name) {
            // Borrow the RefCell and clone the inner HolonCollection
            Rc::new(rc_refcell.borrow().clone())
        } else {
            // Return a new Rc<HolonCollection> if the entry doesn't exist
            Rc::new(HolonCollection::new_staged())
        }
    }

    /// Returns an iterator over the key-value pairs in the map. This is primarily intended for
    /// use by adapters that serialize StagedRelationshipMap into other representations
    /// (e.g., json adapter).
    pub fn iter(&self) -> impl Iterator<Item = (&RelationshipName, &Rc<RefCell<HolonCollection>>)> {
        self.map.iter()
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

    /// Returns `true` if the map contains no relationships.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Serialize for StagedRelationshipMap {
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

impl<'de> Deserialize<'de> for StagedRelationshipMap {
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
