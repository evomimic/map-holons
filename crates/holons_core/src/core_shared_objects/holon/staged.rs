use std::rc::Rc;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::TemporaryId;
use integrity_core_types::{HolonNode, LocalId, PropertyMap, PropertyName, PropertyValue};
use serde::{Deserialize, Serialize};

use crate::{
    core_shared_objects::{
        holon::holon_utils::{key_info, local_id_info},
        ReadableRelationship,
    },
    HolonCollection, HolonError, RelationshipName, StagedRelationshipMap,
};

use super::{
    holon_utils::EssentialHolonContent,
    state::{AccessType, HolonState, StagedState, ValidationState},
    HolonBehavior, TransientHolon,
};

/// Represents a Holon that has been staged for persistence or updates.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StagedHolon {
    version: MapInteger,       // Used to add to hash content for creating TemporaryID
    holon_state: HolonState,   // Mutable or Immutable
    staged_state: StagedState, // ForCreate, ForUpdate, Abandoned, or Committed
    validation_state: ValidationState,
    temporary_id: Option<TemporaryId>, // Ephemeral identifier for staged Holons // RFC4122 UUID
    property_map: PropertyMap,         // Self-describing property data
    staged_relationships: StagedRelationshipMap,
    original_id: Option<LocalId>, // Tracks the predecessor, if cloned from a SavedHolon
    errors: Vec<HolonError>,      // Populated during the commit process
}


impl StagedHolon {
    // =================
    //   CONSTRUCTORS
    // =================

    /// Creates a new StagedHolon in the `ForCreate` state.   
    pub fn new_for_create() -> Self {
        Self {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForCreate,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: PropertyMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: None,
            errors: Vec::new(),
        }
    }

    /// Creates a new StagedHolon in the `ForUpdate` state, linked to a predecessor.
    pub fn new_for_update(original_id: LocalId) -> Self {
        Self {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForUpdate,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: PropertyMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: Some(original_id),
            errors: Vec::new(),
        }
    }

    // ====================
    //    DATA ACCESSORS
    // ====================

    pub fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Use the public `get_related_holons` method on the `StagedRelationshipMap`
        Ok(self.staged_relationships.get_related_holons(relationship_name))
    }

    pub fn get_staged_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.staged_relationships.get_related_holons(relationship_name))
    }

    pub fn get_staged_relationship_map(&self) -> Result<StagedRelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.staged_relationships.clone())
    }

    pub fn get_staged_state(&self) -> StagedState {
        self.staged_state.clone()
    }

    // ==============
    //    MUTATORS
    // ==============

    /// Marks a `StagedHolon` as `Abandoned`.
    ///
    /// # Semantics
    /// - Only applies to `StagedHolons`.
    /// - Ensures the `StagedState` is correctly updated.
    pub fn abandon_staged_changes(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Abandon)?;

        match self.staged_state {
            StagedState::ForCreate | StagedState::ForUpdate | StagedState::ForUpdateChanged => {
                self.staged_state = StagedState::Abandoned;
                self.holon_state = HolonState::Immutable; // Abandoned holons are no longer mutable
                Ok(())
            }
            _ => Err(HolonError::InvalidTransition(
                "Only uncommitted StagedHolons can be abandoned".to_string(),
            )),
        }
    }

    /// Adds an associated HolonError to the errors Vec.
    ///
    /// Used to track all errors obtained during the multi-stage commit process.
    pub fn add_error(&mut self, error: HolonError) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.errors.push(error);
        Ok(())
    }

    /// Marks the `StagedHolon` as `Changed`.
    ///
    /// This is used to transition a `ForUpdate` Holon that has been modified.
    pub fn mark_as_changed(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        if matches!(self.staged_state, StagedState::ForUpdate) {
            self.staged_state = StagedState::ForUpdateChanged;
        }
        Ok(())
    }

    /// Marks the `StagedHolon` as `Committed` and assigns its saved `LocalId`.
    ///
    /// This assumes the Holon has already been persisted to the DHT.
    pub fn to_committed(&mut self, saved_id: LocalId) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Commit)?;

        self.staged_state = StagedState::Committed(saved_id);
        self.holon_state = HolonState::Immutable;
        Ok(())
    }

    pub fn update_staged_state(&mut self, new_state: StagedState) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.staged_state = new_state;
        Ok(())
    }

    pub fn with_property_value(
        &mut self,
        property: PropertyName,
        value: Option<BaseValue>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map.insert(property, value);
        match self.staged_state {
            StagedState::ForUpdate => self.staged_state = StagedState::ForUpdateChanged,
            _ => {}
        }

        Ok(self)
    }
}

// ======================================
//   HOLONBEHAVIOR TRAIT IMPLEMENTATION
// ======================================
impl HolonBehavior for StagedHolon {
    // =====================
    //    DATA ACCESSORS
    // =====================

    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        let mut holon = TransientHolon::new();

        // Retains the predecessor, reference as a LocalId
        holon.update_original_id(self.original_id.clone())?;

        // Copy the existing holon's PropertyMap into the new Holon
        holon.update_property_map(self.property_map.clone())?;

        // Update in place each relationship's HolonCollection State to Staged
        holon.update_relationship_map(self.staged_relationships.clone_for_new_source()?)?;

        Ok(holon)
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(
            self.property_map.clone(),
            self.get_key()?,
            self.errors.clone(),
        ))
    }

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

    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        match &self.staged_state {
            StagedState::Committed(saved_id) => Ok(saved_id.clone()),
            _ => Err(HolonError::EmptyField(
                "Uncommitted StagedHolons do not have LocalIds.".to_string(),
            )),
        }
    }

    fn get_original_id(&self) -> Option<LocalId> {
        self.original_id.clone()
    }

    fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        Ok(self.property_map.get(property_name).cloned().flatten())
    }

    fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .get_key()?
            .ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

        Ok(MapString(key.0 + &self.version.0.to_string()))
    }

    fn into_node(&self) -> HolonNode {
        HolonNode { original_id: self.original_id.clone(), property_map: self.property_map.clone() }
    }

    // =======================
    //     ACCESS CONTROL
    // =======================

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.holon_state {
            HolonState::Mutable => match self.staged_state {
                StagedState::ForCreate | StagedState::ForUpdate | StagedState::ForUpdateChanged => {
                    match access_type {
                        AccessType::Read
                        | AccessType::Write
                        | AccessType::Clone
                        | AccessType::Abandon
                        | AccessType::Commit => Ok(()),
                    }
                }
                StagedState::Abandoned | StagedState::Committed(_) => match access_type {
                    AccessType::Read => Ok(()),
                    _ => Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        "Immutable StagedHolon".to_string(),
                    )),
                },
            },
            HolonState::Immutable => Err(HolonError::NotAccessible(
                format!("{:?}", access_type),
                "Immutable StagedHolon".to_string(),
            )),
        }
    }

    // =================
    //     MUTATORS
    // =================

    fn increment_version(&mut self) -> Result<(), HolonError> {
        self.version.0 += 1;
        Ok(())
    }

    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.original_id = id;
        Ok(())
    }

    fn update_property_map(&mut self, map: PropertyMap) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map = map;
        Ok(())
    }

    // =========================
    //       DIAGNOSTICS
    // =========================
    
    fn debug_info(&self) -> String {
        let phase_info = "StagedHolon";
        let state_info = format!(
            "{} / {}",
            format!("{:?}", self.holon_state),  // Mutable/Immutable
            format!("{:?}", self.staged_state)  // ForCreate, ForUpdate, etc.
        );

        format!("{} / {} / {} / {}", phase_info, state_info, key_info(self), local_id_info(self))
    }

    // =========================
    //       HELPERS
    // =========================

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

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use base_types::{MapBoolean, MapEnumValue};

    use super::*;

    #[test]
    fn instantiate_and_modify() {
        // Initialize default Holon
        let mut initial_holon = StagedHolon::new_for_create();
        let expected_holon = StagedHolon {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForCreate,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: BTreeMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: None,
            errors: Vec::new(),
        };

        assert_eq!(initial_holon, expected_holon);

        // Create example PropertyMap and modify Holon with its update
        let mut property_map = BTreeMap::new();
        let string_property_name = PropertyName(MapString("string property".to_string()));
        let string_value = Some(BaseValue::StringValue(MapString("string_value".to_string())));
        property_map.insert(string_property_name, string_value);
        let boolean_property_name = PropertyName(MapString("boolean property".to_string()));
        let boolean_value = Some(BaseValue::BooleanValue(MapBoolean(true)));
        property_map.insert(boolean_property_name, boolean_value);
        let integer_property_name = PropertyName(MapString("integer property".to_string()));
        let integer_value = Some(BaseValue::IntegerValue(MapInteger(1000)));
        property_map.insert(integer_property_name, integer_value);
        let enum_property_name = PropertyName(MapString("enum property".to_string()));
        let enum_value =
            Some(BaseValue::EnumValue(MapEnumValue(MapString("enum_value".to_string()))));
        property_map.insert(enum_property_name, enum_value);

        initial_holon.update_property_map(property_map.clone()).unwrap();

        assert_eq!(initial_holon.property_map, property_map);
    }

    #[test]
    fn property_management() {
        let mut holon = StagedHolon::new_for_create();
        let property_name = PropertyName(MapString("first property".to_string()));
        // Add a value to the property map
        let initial_value = BaseValue::IntegerValue(MapInteger(1));
        holon.with_property_value(property_name.clone(), Some(initial_value.clone())).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), Some(initial_value));
        // Update value for the same property name
        let changed_value = BaseValue::StringValue(MapString("changed value".to_string()));
        holon.with_property_value(property_name.clone(), Some(changed_value.clone())).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), Some(changed_value));
        // Remove value by updating to None
        holon.with_property_value(property_name.clone(), None).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), None);
    }

    // #[test]
    // fn relationship_management() {
    //     let mut holon = StagedHolon::new_for_create();
    //     let relationship_name = RelationshipName(MapString("first relationship".to_string()));
    //     let first_collection = HolonCollection::new_existing();
    //     //

    //     // // Add a relationship to the relationship map
    //     // holon.add_related_holon(relationship_name.clone(), Some(initial_value));
    //     // assert_eq!();
    //     // // Update relationship value for the same relationship name
    //     // let changed_collection = BaseValue::StringValue(MapString("changed value".to_string()));
    //     // holon.remove_related_holon();
    //     // assert_eq!();

    //     // TODO: remove relationship
    // }

    #[test]
    fn verify_default_values() {
        let default_holon = StagedHolon::new_for_create();
        let expected_holon = StagedHolon {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForCreate,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: BTreeMap::new(),
            staged_relationships: StagedRelationshipMap { map: BTreeMap::new() },
            original_id: None,
            errors: Vec::new(),
        };

        assert_eq!(default_holon, expected_holon);
    }
}
