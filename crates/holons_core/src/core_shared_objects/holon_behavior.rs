use std::sync::{Arc, RwLock};

use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyName, PropertyValue, RelationshipName,
};

use crate::{
    core_shared_objects::holon::{state::AccessType, EssentialHolonContent, HolonCloneModel},
    HolonCollection, HolonReference, HolonsContextBehavior, RelationshipMap,
};

/// The `ReadableHolonState` trait mirrors ReadableHolon, so that references can simply delegate without having to match Holon variant.
/// It also provides methods that are shared internally but not part of the WriteableHolon trait. Holon must pass is_accessibile check for Write.
pub trait ReadableHolonState {
    /// Gets all related Holons.
    ///
    /// Returns a generic RelationshipMap (HashMap)
    fn all_related_holons(&self) -> Result<RelationshipMap, HolonError>;

    /// Extracts the core data content for comparison, validation, or lightweight inspection.
    ///
    /// Includes property data and key information, but excludes phase-specific metadata like
    /// `StagedState`, `HolonState`, or `SavedState`.
    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError>;

    /// Converts a Holon into a HolonCloneModel.
    ///
    ///  # Semantics
    ///  -Extracts version, original_id, properties, and relationships
    fn holon_clone_model(&self) -> HolonCloneModel;

    /// Only applies for StagedHolons in StagedState::Committed, otherwise throws an error.
    fn holon_id(&self) -> Result<HolonId, HolonError>;

    /// Converts a Holon into a HolonNode.
    ///
    ///  # Semantics
    ///  -Extracts original_id and property_map
    fn into_node_model(&self) -> HolonNodeModel;

    /// Retrieves the Holon's primary key value (if present).
    ///
    /// # Semantics
    /// - Keys are typically defined in the `property_map` as `"key"`.
    /// - Not all Holons have keys; `None` may be returned.
    fn key(&self) -> Result<Option<MapString>, HolonError>;

    /// Retrieves the `original_id` of the Holon, representing its predecessor.
    ///
    /// # Semantics
    /// - **`TransientHolons`** may have an `original_id` if cloned from a `SavedHolon`.  
    /// - **`StagedHolons`** may have an `original_id` if cloned as part of a `ForUpdate` cycle.  
    /// - **`SavedHolons`** may have an `original_id` if they are a version of a prior Holon.  
    /// - New Holons created without cloning will have `None` as their `original_id`.
    fn original_id(&self) -> Option<LocalId>;

    /// Retrieves the specified property value from the Holon.
    ///
    /// # Semantics
    /// - Returns `Ok(None)` if the property exists but has a `None` value.  
    /// - Returns `Ok(None)` if the property does **not exist at all**.  
    ///
    /// **Note:** To differentiate between a `None` value and a missing property,  
    /// clients should use `.property_value()` along with `.essential_content()`.
    fn property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError>;

    /// Gets a related holons for a given relationship name.
    ///
    /// Returns a HolonCollection where its members are the related HolonReferences.
    fn related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError>;

    /// Retrieves the unique versioned key (key + versioned_sequence_count suffix)
    ///
    /// # Semantics
    /// - The versioned key is used for identifying Holons in the Nursery where multiple have been staged with the same base key.
    /// - Returns error if the Holon does not have a key, since that is required for this function call.
    ///
    /// # Errors
    /// - Returns `Err(HolonError::InvalidParameter)` if the Holon does not have a key.
    fn versioned_key(&self) -> Result<MapString, HolonError>;

    /// Enforces access control rules for the Holon’s current phase and state.
    ///
    /// # Semantics
    /// - **`TransientHolons`** are mutable unless marked immutable.
    /// - **`StagedHolons`** become immutable when committed or abandoned.
    /// - **`SavedHolons`** are always immutable (except for deletion metadata).
    ///
    /// # Errors
    /// Returns `Err(HolonError::NotAccessible)` if the requested access type is disallowed.
    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError>;

    /// Returns a String summary of the Holon.
    ///
    /// -Only used for logging. Provides a more concise message to avoid log bloat.
    fn summarize(&self) -> String;
}

/// The `WriteableHolonState` trait mirrors WriteableHolon so that references can simply delegate without having to match Holon variant.
/// It also provides methods that are shared internally but not part of the WriteableHolon trait. Holon must pass is_accessibile check for Write.
pub trait WriteableHolonState {
    /// Inserts a HolonCollection of related HolonReferences for the given relationship name.
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError>;

    /// Called by HolonPool::insert_holon() to increment the version.
    ///
    /// Used to track ephemeral versions of Holons with the same key.
    fn increment_version(&mut self) -> Result<(), HolonError>;

    /// Marks the Holon as immutable.
    ///
    /// Used in scenarios where immutability must be enforced post-creation.
    fn mark_as_immutable(&mut self) -> Result<(), HolonError>;

    /// Removes a property from the Holon's 'property_map'.
    fn remove_property_value(&mut self, property: &PropertyName) -> Result<&mut Self, HolonError>;

    /// Removes the HolonCollection of related HolonReferences for the given relationship name.
    fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError>;

    /// Replaces the 'OriginalId' with the provided optional 'LocalId'.
    ///
    /// Used when cloning a Holon to retain predecessor.
    /// Called by stage_new_from_clone to reset predecessor to None.
    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError>;

    /// Adds a property to the Holon's 'property_map' if one does not exist with the given name,
    /// otherwise replaces the value of such.
    fn with_property_value(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError>;
}
