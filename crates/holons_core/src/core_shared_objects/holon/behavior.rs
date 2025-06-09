use shared_types_holon::{HolonNode, LocalId, MapString, PropertyMap, PropertyName, PropertyValue};

use crate::HolonError;

use super::{holon_utils::EssentialHolonContent, state::AccessType, TransientHolon};

/// The `HolonBehavior` trait defines the core interface for interacting with Holon instances,
/// including data access, lifecycle control, and diagnostic capabilities.
pub trait HolonBehavior {
    // =====================
    //     CONSTRUCTORS
    // =====================

    /// Clones the Holon as a new `TransientHolon`.
    ///
    /// Regardless of the source phase, cloned Holons are always `TransientHolons`.
    fn clone_holon(&self) -> Result<TransientHolon, HolonError>;

    // =======================
    //     DATA ACCESSORS
    // =======================

    /// Extracts the core data content for comparison, validation, or lightweight inspection.
    ///
    /// Includes property data and key information, but excludes phase-specific metadata like
    /// `StagedState`, `HolonState`, or `SavedState`.
    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError>;

    /// Retrieves the Holon's primary key value (if present).
    ///
    /// # Semantics
    /// - Keys are typically defined in the `property_map` as `"key"`.
    /// - Not all Holons have keys; `None` may be returned.
    fn get_key(&self) -> Result<Option<MapString>, HolonError>;

    /// Retrieves the Holon's `LocalId`, which identifies it within the Holochain network.
    ///
    /// # Semantics
    /// - **`SavedHolons`** always have a `LocalId`.  
    /// - **`StagedHolons`** have a `LocalId` **only if** they have been committed.  
    /// - **`TransientHolons`** do not have a `LocalId`.  
    ///
    /// # Errors
    /// - Returns `Err(HolonError::EmptyField)` if the Holon phase doesn’t support a `LocalId`.
    fn get_local_id(&self) -> Result<LocalId, HolonError>;

    /// Retrieves the `original_id` of the Holon, representing its predecessor.
    ///
    /// # Semantics
    /// - **`TransientHolons`** may have an `original_id` if cloned from a `SavedHolon`.  
    /// - **`StagedHolons`** may have an `original_id` if cloned as part of a `ForUpdate` cycle.  
    /// - **`SavedHolons`** may have an `original_id` if they are a version of a prior Holon.  
    /// - New Holons created without cloning will have `None` as their `original_id`.
    fn get_original_id(&self) -> Option<LocalId>;

    /// Retrieves the specified property value from the Holon.
    ///
    /// # Semantics
    /// - Returns `Ok(None)` if the property exists but has a `None` value.  
    /// - Returns `Ok(None)` if the property does **not exist at all**.  
    ///
    /// **Note:** To differentiate between a `None` value and a missing property,  
    /// clients should use `.get_property_value()` along with `.essential_content()`.
    fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError>;

    /// Retrieves the unique versioned key (key + versioned_sequence_count suffix)
    ///
    /// # Semantics
    /// - The versioned key is used for identifying Holons in the Nursery where multiple have been staged with the same base key.
    /// - Returns error if the Holon does not have a key, since that is required for this function call.
    ///
    /// # Errors
    /// - Returns `Err(HolonError::InvalidParameter)` if the Holon does not have a key.
    fn get_versioned_key(&self) -> Result<MapString, HolonError>;

    /// Converts a Holon into a HolonNode.
    ///
    ///  # Semantics
    ///  -Extracts property_map and original_id fields
    fn into_node(&self) -> HolonNode;

    // =========================
    //      ACCESS CONTROL
    // =========================

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

    // =================
    //     MUTATORS
    // =================

    /// Called by HolonPool::insert_holon() to increment the version.
    ///
    /// Used to track ephemeral versions of Holons with the same key.
    fn increment_version(&mut self) -> Result<(), HolonError>;

    /// **TODO DOC: Updates
    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError>;

    /// **TODO DOC: Updates
    fn update_property_map(&mut self, map: PropertyMap) -> Result<(), HolonError>;

    // =========================
    //       DIAGNOSTICS
    // =========================

    /// Provides structured diagnostic details about the Holon’s phase, state, and key metadata.
    ///
    /// # Usage
    /// Designed for debugging, tracing, or visualizing Holon state.
    ///

    fn debug_info(&self) -> String;

    // ==================
    //      HELPERS
    // ==================

    /// Returns a String summary of the Holon.
    ///
    /// -Only used for logging. Provides a more concise message to avoid log bloat.
    fn summarize(&self) -> String;
}
