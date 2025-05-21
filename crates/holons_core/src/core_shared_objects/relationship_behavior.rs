use std::rc::Rc;

use crate::{HolonReference, HolonsContextBehavior};

use super::{holon::state::AccessType, HolonCollection, HolonError, RelationshipName};




pub trait ReadableRelationship {
    // =====================
    //     CONSTRUCTORS
    // =====================

    /// Clones the Relationship Map for a new source. The `HolonCollection` objects are also cloned
    /// for the new source using their `clone_for_new_source` method.
    ///
    /// # Returns
    /// - `Ok( Relationship Map trait object with cloned `HolonCollection` objects.)
    /// - `Err(HolonError)`: If cloning any `HolonCollection` fails.
    fn clone_for_new_source(&self) -> Result<Box<dyn ReadableRelationship>, HolonError>;

    // ====================
    //    DATA ACCESSORS
    // ====================

    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Rc`.
    ///
    /// If the `relationship_name` exists in the Relationship Map, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    /// Retrieves the `HolonCollection` for the given relationship name, wrapped in `Rc`.
    ///
    /// If the `relationship_name` exists in the Relationship Map, this method returns the
    /// corresponding collection wrapped in an `Rc`. If the relationship is not found, an empty
    /// `HolonCollection` wrapped in an `Rc` is returned instead.
    fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection>;


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
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
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
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;



}