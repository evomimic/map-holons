use crate::core_shared_objects::{HolonCollection, HolonError, RelationshipName};
use hdk::prelude::*;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::rc::Rc;

/// Represents a map of staged relationships, where the keys are relationship names and the values
/// are fully-loaded collections of holons for those relationships. Absence of an entry indicates
/// that the relationship has no associated holons.
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct StagedRelationshipMap(pub BTreeMap<RelationshipName, Rc<HolonCollection>>);
impl StagedRelationshipMap {
    /// Creates a new, empty `StagedRelationshipMap`.
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Clones the `StagedRelationshipMap` for a new source. The `HolonCollection` objects are also cloned
    /// for the new source using their `clone_for_new_source` method.
    ///
    /// # Returns
    /// - `Ok(Self)`: A new `StagedRelationshipMap` with cloned `HolonCollection` objects.
    /// - `Err(HolonError)`: If cloning any `HolonCollection` fails.
    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        let mut cloned_relationship_map = BTreeMap::new();

        for (name, collection) in &self.0 {
            let cloned_collection = collection.clone_for_new_source()?; // Assumes `clone_for_new_source` exists on `HolonCollection`.
            cloned_relationship_map.insert(name.clone(), Rc::new(cloned_collection));
        }

        Ok(StagedRelationshipMap(cloned_relationship_map))
    }

    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Rc`.
    ///
    /// If the `relationship_name` exists in the `StagedRelationshipMap`, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    pub fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection> {
        // Return the existing collection if found, otherwise return an empty HolonCollection.
        self.0
            .get(relationship_name)
            .cloned() // Convert &Rc<HolonCollection> to Rc<HolonCollection>
            .unwrap_or_else(|| Rc::new(HolonCollection::new_staged())) // Default to empty collection
    }

    /// Inserts a new relationship into the map, replacing any existing collection for that relationship.
    ///
    /// # Arguments
    /// - `relationship_name`: The name of the relationship to insert or update.
    /// - `collection`: The `HolonCollection` to associate with the relationship.
    pub fn insert(&mut self, relationship_name: RelationshipName, collection: HolonCollection) {
        self.0.insert(relationship_name, Rc::new(collection));
    }

    /// Removes a relationship from the map.
    ///
    /// # Arguments
    /// - `relationship_name`: The name of the relationship to remove.
    pub fn remove(&mut self, relationship_name: &RelationshipName) {
        self.0.remove(relationship_name);
    }

    /// Returns `true` if the map contains no relationships.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Deref implementation for `StagedRelationshipMap`, dereferencing to the underlying `BTreeMap`.
impl Deref for StagedRelationshipMap {
    type Target = BTreeMap<RelationshipName, Rc<HolonCollection>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
