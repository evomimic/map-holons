
use serde::{Deserialize, Serialize};
use shared_types_holon::{LocalId, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue};

use crate::{
    core_shared_objects::holon::holon_utils::{key_info, local_id_info},
    HolonError,
};

use super::{
    holon_utils::EssentialHolonContent, saved_holon_node::SavedHolonNode, state::{AccessType, HolonState, SavedState, ValidationState}, HolonBehavior, TransientHolon
};

/// Represents a Holon that has been persisted in the DHT.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SavedHolon {
    holon_state: HolonState,           // Always `Immutable`
    validation_state: ValidationState, //
    saved_node: SavedHolonNode,        // Links to persisted Holon data
    version: MapInteger, 
    saved_state: SavedState,
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
        saved_node: SavedHolonNode,
        property_map: PropertyMap,
        original_id: Option<LocalId>,
        version: MapInteger,
    ) -> Self {
        Self {
            holon_state: HolonState::Immutable,
            validation_state: ValidationState::ValidationRequired,
            saved_node,
            version,
            saved_state: SavedState::Fetched,
            property_map,
            original_id,
        }
    }
}


// ==================================
//    HOLONBEHAVIOR IMPLEMENTATION
// ==================================
impl HolonBehavior for SavedHolon {
    // =====================
    //    DATA ACCESSORS
    // =====================

    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        let mut holon = TransientHolon::new();

        // Retains the predecessor node, referenced by LocalId
        holon.update_original_id(Some(self.saved_node.get_local_id())); 

        // Copy the existing holon's PropertyMap into the new Holon
        holon.update_property_map(self.property_map.clone());


        Ok(holon)
    }

    /// Extracts essential content for comparison or testing.
    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(self.property_map.clone(), self.get_key()?, Vec::new()))
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

    /// Retrieves the unique versioned key (key property value + semantic version)
    ///
    /// # Semantics
    /// - The versioned key is used for identifying Holons in the Nursery where multiple have been staged with the same base key.
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

    /// Retrieves the `LocalId` from the `SavedHolonNode`.
    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        Ok(self.saved_node.get_local_id())
    }

    /// Retrieves the `original_id`, if present.
    fn get_original_id(&self) -> Result<Option<LocalId>, HolonError> {
        Ok(self.original_id.clone())
    }

    /// Retrieves the specified property value.
    fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        Ok(self.property_map.get(property_name).cloned().flatten())
    }

    // =========================
    //     ACCESS CONTROL
    // =========================

    /// Enforces access control rules for `SavedHolon`.
    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match access_type {
            AccessType::Read | AccessType::Clone => Ok(()), // Always accessible for reading/cloning
            AccessType::Write | AccessType::Commit | AccessType::Abandon => {
                Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    "Immutable SavedHolon".to_string(),
                ))
            }
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


    // =========================
    //       DIAGNOSTICS
    // =========================

    /// Provides structured diagnostic information about the Holon's phase and state.
    fn debug_info(&self) -> String {
        let phase_info = "SavedHolon";
        let state_info = format!(
            "{} / {}",
            format!("{:?}", self.holon_state), // Immutable/Deleted
            format!("{:?}", self.saved_state)  // Fetched, Deleted, etc.
        );

        format!("{} / {} / {} / {}", phase_info, state_info, key_info(self), local_id_info(self))
    }

    // =========================
    //       HELPERS
    // =========================

    /// Returns a String summary of the Holon.
    fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.get_key() {
            Ok(Some(key)) => key.0,  // Extract the key from MapString
            Ok(None) => "<None>".to_string(),   // Key is None
            Err(_) => "<Error>".to_string(),    // Error encountered while fetching key
        };

        // Attempt to extract local_id using get_local_id method, default to "None" if not available
        let local_id = match self.get_local_id() {
            Ok(local_id) => local_id.0.to_string(), // Convert LocalId to String
            Err(e) => format!("<Error: {:?}>", e),  // If local_id is not found or error occurred
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
            key, local_id, self.holon_state, self.validation_state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core_shared_objects::holon::saved_holon_node::SavedHolonNode, HolonError};
    use hdk::prelude::{ActionHash, Record};

    /// Utility function to create a mock `Record`.
    fn mock_record(hash_str: &str) -> Record {
        let action_hash = ActionHash::from_raw_32(hash_str.as_bytes());
        Record::new(action_hash)
    }

    #[test]
    fn get_local_id_returns_local_id_when_saved_node_is_present() {
        // Arrange
        let record = mock_record("valid_hash");
        let saved_holon_node = SavedHolonNode { saved_node: Some(record.clone()) };

        // Act
        let result = saved_holon_node.get_local_id();

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), LocalId(record.action_address().clone()));
    }

    #[test]
    fn get_local_id_returns_error_when_saved_node_is_none() {
        // Arrange
        let saved_holon_node = SavedHolonNode { saved_node: None };

        // Act
        let result = saved_holon_node.get_local_id();

        // Assert
        assert!(result.is_err());
        assert!(matches!(result, Err(HolonError::HolonNotFound(_))));
    }

    #[test]
    fn get_local_id_produces_correct_error_message() {
        // Arrange
        let saved_holon_node = SavedHolonNode { saved_node: None };

        // Act
        let result = saved_holon_node.get_local_id();

        // Assert
        match result {
            Err(HolonError::HolonNotFound(msg)) => {
                assert_eq!(msg, "SavedHolonNode is empty");
            }
            _ => panic!("Expected HolonNotFound error with correct message"),
        }
    }

    #[test]
    fn get_local_id_with_different_records_produces_correct_ids() {
        // Arrange
        let record1 = mock_record("hash_one");
        let record2 = mock_record("hash_two");

        let node1 = SavedHolonNode { saved_node: Some(record1.clone()) };

        let node2 = SavedHolonNode { saved_node: Some(record2.clone()) };

        // Act & Assert
        assert_eq!(node1.get_local_id().unwrap(), LocalId(record1.action_address().clone()));
        assert_eq!(node2.get_local_id().unwrap(), LocalId(record2.action_address().clone()));
        assert_ne!(node1.get_local_id().unwrap(), node2.get_local_id().unwrap());
    }
}
