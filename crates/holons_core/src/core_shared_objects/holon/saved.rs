use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use base_types::{MapInteger, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue,
    RelationshipName,
};

use crate::{
    core_shared_objects::{holon::HolonCloneModel, holon_behavior::ReadableHolonState},
    HolonCollection, RelationshipMap,
};

use super::{
    state::{AccessType, HolonState, SavedState, ValidationState},
    EssentialHolonContent,
};

/// Represents a Holon that has been persisted in the DHT.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SavedHolon {
    holon_state: HolonState,           // Always `Immutable`
    validation_state: ValidationState, //
    saved_id: LocalId,                 // Links to persisted Holon data
    version: MapInteger,
    saved_state: SavedState,
    // HolonNode data:
    property_map: PropertyMap,    // Self-describing property data
    original_id: Option<LocalId>, // Tracks predecessor, if applicable
}

// ================================
//    ASSOCIATED METHODS (IMPL BLOCK)
// ================================
impl SavedHolon {
    /// Creates a new `SavedHolon` in the `Immutable` state.
    ///
    /// This method is called during inflation from a `HolonNode`.
    pub fn new(
        saved_id: LocalId,
        property_map: PropertyMap,
        original_id: Option<LocalId>,
        version: MapInteger,
    ) -> Self {
        Self {
            holon_state: HolonState::Immutable,
            validation_state: ValidationState::ValidationRequired,
            saved_id,
            version,
            saved_state: SavedState::Fetched,
            property_map,
            original_id,
        }
    }

    pub fn holon_clone_model(&self) -> HolonCloneModel {
        HolonCloneModel::new(
            self.version.clone(),
            self.original_id.clone(),
            self.property_map.clone(),
            None,
        )
    }

    /// Retrieves the `LocalId`.
    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        Ok(self.saved_id.clone())
    }
}

// ================================================
//    HOLONBEHAVIOR- READABLE ONLY IMPLEMENTATION
// ================================================
impl ReadableHolonState for SavedHolon {
    fn all_related_holons(&self) -> Result<RelationshipMap, HolonError> {
        Err(HolonError::NotImplemented(
            "Must go through reference layer for getting relationships".to_string(),
        ))
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(self.property_map.clone(), self.key()?, Vec::new()))
    }

    fn holon_clone_model(&self) -> HolonCloneModel {
        HolonCloneModel::new(
            self.version.clone(),
            self.original_id.clone(),
            self.property_map.clone(),
            None,
        )
    }

    fn holon_id(&self) -> Result<HolonId, HolonError> {
        Err(HolonError::NotImplemented(
            "Must go through reference layer for getting HolonId from SmartReference".to_string(),
        ))
    }

    /// Retrieves the Holon's primary key, if defined in its `property_map`.
    fn key(&self) -> Result<Option<MapString>, HolonError> {
        if let Some(inner_value) =
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

    /// Retrieves the `original_id`, if present.
    fn original_id(&self) -> Option<LocalId> {
        self.original_id.clone()
    }

    /// Retrieves the specified property value.
    fn property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        Ok(self.property_map.get(property_name).cloned())
    }

    fn related_holons(
        &self,
        _relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        Err(HolonError::NotImplemented(
            "Must go through reference layer for getting relationships".to_string(),
        ))
    }

    // ?TODO:  What should this be for SavedHolon ? Return error ?
    // not sure why we need a version for this type
    //
    /// Retrieves the unique versioned key (key property value + semantic version)
    ///
    /// # Semantics
    /// - The versioned key is used for identifying Holons in the Nursery where multiple have been staged with the same base key.
    /// - Returns error if the Holon does not have a key, since that is required for this function call.
    ///
    /// # Errors
    /// - Returns `Err(HolonError::InvalidParameter)` if the Holon does not have a key.
    fn versioned_key(&self) -> Result<MapString, HolonError> {
        let key =
            self.key()?.ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

        Ok(MapString(key.0 + &self.version.0.to_string()))
    }

    /// Extracts HolonNode data.
    /// Converts 'original_id' and 'property_map' fields into a HolonNode object.
    fn into_node_model(&self) -> HolonNodeModel {
        HolonNodeModel::new(self.original_id.clone(), self.property_map.clone())
    }

    /// Enforces access control rules for `SavedHolon`.
    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match access_type {
            AccessType::Read | AccessType::Clone => Ok(()), // Always accessible for reading/cloning
            AccessType::Write | AccessType::Commit | AccessType::Abandon => {
                Err(HolonError::NotAccessible(format!("{:?}", access_type), "Saved".to_string()))
            }
        }
    }

    fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.key() {
            Ok(Some(key)) => key.0,           // Extract the key from MapString
            Ok(None) => "<None>".to_string(), // Key is None
            Err(_) => "<Error>".to_string(),  // Error encountered while fetching key
        };

        // Attempt to extract local_id using get_local_id method, default to "None" if not available
        let local_id = match self.get_local_id() {
            Ok(local_id) => local_id.to_string(), // Convert LocalId to String
            Err(e) => format!("<Error: {:?}>", e), // If local_id is not found or error occurred
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
            key, local_id, self.holon_state, self.validation_state
        )
    }
}
