
// use crate::holon::behavior::HolonBehavior;
// use crate::common::{PropertyName, PropertyValue, EssentialHolonContent, MapString};
// use crate::holon::{HolonError, HolonState};
// use crate::state::AccessType;
// use crate::identifier::TemporaryId;

use serde::{Deserialize, Serialize};
use shared_types_holon::{LocalId, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue, TemporaryId};

use crate::{core_shared_objects::{holon::holon_utils::{key_info, local_id_info}, TransientRelationshipMap}, HolonError};

use super::{holon_utils::EssentialHolonContent, state::{AccessType, HolonState, ValidationState}, HolonBehavior};


/// Represents a Holon that exists only in-memory and is never intended for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct TransientHolon {
    version: MapInteger, // Used to add to hash content for creating TemporaryID
    holon_state: HolonState,            // Mutable or Immutable
    validation_state: ValidationState, 
    temporary_id: Option<TemporaryId>,  // Ephemeral identifier for TransientHolons
    property_map: PropertyMap,          // Self-describing property data        
    transient_relationships: TransientRelationshipMap, // Tracks ephemeral relationships
    original_id: Option<LocalId>,       // Tracks the predecessor, if cloned from a SavedHolon
}

// ==================================
//   ASSOCIATED METHODS (IMPL BLOCK)
// ==================================
impl TransientHolon {

    // ================
    //   CONSTRUCTORS
    // ================

    /// Creates a new, mutable `TransientHolon`.
    pub fn new() -> Self {
        Self {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: PropertyMap::new(),
            transient_relationships: TransientRelationshipMap::new(),
            original_id: None,
        }
    }

    /// Creates a new, immutable `TransientHolon`.
    ///
    /// This is used when deserializing TransientHolons outside their originating environment.
    pub fn new_immutable() -> Self {
        Self {
            version: MapInteger(1),
            holon_state: HolonState::Immutable,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: PropertyMap::new(),
            transient_relationships: TransientRelationshipMap::new(),
            original_id: None,
        }
    }

    // ==============
    //    MUTATORS
    // ==============

    /// Marks the `TransientHolon` as immutable.
    ///
    /// Used in scenarios where immutability must be enforced post-creation.
    pub fn mark_as_immutable(&mut self) {
        self.holon_state = HolonState::Immutable;
    }

    pub fn update_relationship_map(&mut self, map: TransientRelationshipMap) {
        self.transient_relationships = map;
    }

}

// ================================
//   HOLONBEHAVIOR IMPLEMENTATION
// ================================
impl HolonBehavior for TransientHolon {
    // ====================
    //    DATA ACCESSORS
    // ====================

    // /// Clone an existing Holon and return a Holon that can be staged for building and eventual commit. ***
    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        
        let mut holon = TransientHolon::new();

        // Copy the existing holon's PropertyMap into the new Holon
        holon.property_map = self.property_map.clone();

        // Update in place each relationship's HolonCollection State to Staged
        holon.transient_relationships = self.transient_relationships.clone_for_new_source()?;

        Ok(holon)

    }

    /// Extracts essential content for comparison or testing.
    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(
            self.property_map.clone(),
            self.get_key()?,
            Vec::new(), // defaulting to empty
        ))
    }

    /// Retrieves the Holon's primary key, if defined in its `property_map`.
    fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        if let Some(Some(inner_value)) =
            self.property_map.get(&PropertyName(MapString("key".to_string())))
        {
            let string_value: String = inner_value.try_into().map_err(|_| {
                HolonError::UnexpectedValueType(
                    format!("{:?}", inner_value),
                    "MapString".to_string(),
                )
            })?;
            Ok(Some(MapString(string_value)))
        } else {
            Ok(None)
        }
    }

    /// Retrieves the unique versioned_key (key property value + versioned suffix).
    ///
    /// # Semantics
    /// - 
    /// - Returns error if the Holon does not have a key, since that is required for this function call.
    ///
    /// # Errors
    /// - Returns `Err(HolonError::InvalidParameter)` if the Holon does not have a key.
    fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .get_key()?
            .ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

        Ok(MapString(key.0 + &self.version.0.to_string()))
    }

    /// `TransientHolons` do not have a `LocalId`.
    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        Err(HolonError::EmptyField("TransientHolons do not have LocalIds.".to_string()))
    }

    /// Retrieves the `original_id`, if present.
    fn get_original_id(&self) -> Result<Option<LocalId>, HolonError> {
        Ok(self.original_id.clone())
    }

    /// Retrieves the specified property value.
    fn get_property_value(&self, property_name: &PropertyName) -> Result<Option<PropertyValue>, HolonError> {
        Ok(self.property_map.get(property_name).cloned().flatten())
    }

    // =========================
    //     ACCESS CONTROL
    // =========================

    /// Enforces access control rules for `TransientHolon` states.
    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.holon_state {
            HolonState::Mutable => match access_type {
                AccessType::Read
                | AccessType::Write
                | AccessType::Clone
                | AccessType::Abandon => Ok(()),
                AccessType::Commit => Err(HolonError::InvalidTransition(
                    "TransientHolons cannot be committed.".to_string(),
                )),
            },
            HolonState::Immutable => match access_type {
                AccessType::Read | AccessType::Clone => Ok(()),
                AccessType::Write | AccessType::Commit | AccessType::Abandon => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        "Immutable TransientHolon".to_string(),
                    ))
                }
            },
        }
    }

    // =================
    //     MUTATORS
    // =================

    fn increment_version(&mut self) {
        self.version.0 += 1;
    }

    fn update_original_id(&mut self, id: Option<LocalId>) {
        self.original_id = id;
    }

    fn update_property_map(&mut self, map: PropertyMap) {
        self.property_map = map;
    }

    // =======================
    //       DIAGNOSTICS
    // =======================

      fn debug_info(&self) -> String {
        let phase_info = "TransientHolon";
        let state_info = format!("{:?}", self.holon_state);  // Directly shows Mutable/Immutable

        format!(
            "{} / {} / {} / {}",
            phase_info,
            state_info,
            key_info(self),
            local_id_info(self)
        )
    }

    // ===================
    //       HELPERS
    // ===================

    /// Returns a String summary of the Holon.
    fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.get_key() {
            Ok(Some(key)) => key.0,           // Extract the key from MapString
            Ok(None) => "<None>".to_string(), // Key is None
            Err(_) => "<Error>".to_string(),  // Error encountered while fetching key
        };

        // Attempt to extract local_id using get_local_id method, default to "None" if not available
        let local_id = match self.get_local_id() {
            Ok(local_id) => local_id.0.to_string(), // Convert LocalId to String
            Err(_) => "<None>".to_string(),         // If local_id is not found or error occurred
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
            key, local_id, self.holon_state, self.validation_state
        )
    }
}