use super::{HolonCollection, TransientRelationshipMap};
use crate::HolonReference;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};
use std::sync::{Arc, RwLock};

pub trait ReadableRelationship {
    // =====================
    //     CONSTRUCTORS
    // =====================

    /// Clones the Relationship Map for a new source. The `HolonCollection` objects are also cloned
    /// for the new source using their `clone_for_new_source` method.
    ///
    /// # Returns
    /// - `Ok( TransientRelationshipMap with cloned `HolonCollection` objects.)
    /// - `Err(HolonError)`: If cloning any `HolonCollection` fails.
    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError>;

    // ====================
    //    DATA ACCESSORS
    // ====================

    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Arc<RwLock<HolonCollection>>`.
    ///
    /// If the `relationship_name` exists in the Relationship Map, this method returns the
    /// corresponding collection wrapped in `Arc<RwLock<HolonCollection>>`. If the relationship
    /// is not found, an empty `HolonCollection` wrapped in `Arc<RwLock<HolonCollection>>` is returned instead.
    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Arc<RwLock<HolonCollection>>`.
    ///
    /// If the `relationship_name` exists in the Relationship Map, this method returns the
    /// corresponding collection wrapped in `Arc<RwLock<HolonCollection>>`. If the relationship
    /// is not found, an empty `HolonCollection` wrapped in `Arc<RwLock<HolonCollection>>` is returned instead.
    // TODO(PERF/CLEANUP): This clones the inner HolonCollection, which may be costly in time/space.
    // Ideally, the trait method would return an associated type (e.g., `type Output: Clone`) to allow
    // different implementations to return shared ownership types like `Rc<RefCell<_>>`.
    // For now, we’re preserving this behavior to avoid destabilizing the long-lived feature branch;
    // revisit after merge for a more efficient and idiomatic design.
    fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Arc<RwLock<HolonCollection>>;
}

pub trait WritableRelationship {
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
    fn add_related_holons(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    /// Adds holon references with precomputed keys, avoiding key lookups during mutation.
    fn add_related_holons_with_keys(
        &mut self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;

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
    fn remove_related_holons(
        &mut self,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    /// Removes holon references with precomputed keys, avoiding key lookups during mutation.
    fn remove_related_holons_with_keys(
        &mut self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;
}
