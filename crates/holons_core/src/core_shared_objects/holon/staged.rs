use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

use crate::{
    core_shared_objects::{
        holon::{
            AccessType, EssentialHolonContent, HolonCloneModel, HolonState, StagedState,
            ValidationState,
        },
        ReadableHolonState, ReadableRelationship, WritableRelationship, WriteableHolonState,
    },
    HolonCollection, HolonReference, HolonsContextBehavior, RelationshipMap, StagedRelationshipMap,
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue,
    RelationshipName,
};
use type_names::CorePropertyTypeName;

/// Represents a Holon that has been staged for persistence or updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedHolon {
    version: MapInteger,       // Used to add to hash content for creating TemporaryID
    holon_state: HolonState,   // Mutable or Immutable
    staged_state: StagedState, // ForCreate, ForUpdate, Abandoned, or Committed
    validation_state: ValidationState,
    property_map: PropertyMap, // Self-describing property data
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
            property_map: PropertyMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: None,
            errors: Vec::new(),
        }
    }

    /// Creates a new StagedHolon in the `ForCreate` state.   
    pub fn new_from_clone_model(model: HolonCloneModel) -> Result<Self, HolonError> {
        let staged_relationships: StagedRelationshipMap = {
            if let Some(relationship_map) = model.relationships {
                relationship_map.clone_for_staged()?
            } else {
                return Err(HolonError::InvalidParameter("HolonCloneModel passed through this constructor must always contain a RelationshipMap, even if empty".to_string()));
            }
        };
        let staged_holon = Self {
            version: model.version,
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForCreate,
            validation_state: ValidationState::ValidationRequired,
            property_map: model.properties,
            staged_relationships,
            original_id: model.original_id,
            errors: Vec::new(),
        };

        Ok(staged_holon)
    }

    /// Creates a new StagedHolon in the `ForUpdate` state, linked to a predecessor.
    pub fn new_for_update(original_id: LocalId) -> Self {
        Self {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForUpdate,
            validation_state: ValidationState::ValidationRequired,
            property_map: PropertyMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: Some(original_id),
            errors: Vec::new(),
        }
    }

    // ==================
    //   DATA ACCESSORS
    // ==================

    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        match &self.staged_state {
            StagedState::Committed(saved_id) => Ok(saved_id.clone()),
            _ => Err(HolonError::EmptyField(
                "Uncommitted StagedHolons do not have LocalIds.".to_string(),
            )),
        }
    }

    pub fn get_staged_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
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

    pub fn mark_as_immutable(&mut self) -> Result<(), HolonError> {
        self.holon_state = HolonState::Immutable;
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

    /// Replaces the 'OriginalId' with the provided optional 'LocalId'.
    ///
    /// Used when cloning a Holon to retain predecessor.
    /// Called by stage_new_from_clone to reset predecessor to None.
    pub fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.original_id = id;
        Ok(())
    }

    // /// Updates the 'StagedState'
    // ///
    // /// Used when
    // /// Called by
    // pub fn update_staged_state(&mut self, new_state: StagedState) -> Result<(), HolonError> {
    //     self.is_accessible(AccessType::Write)?;
    //     self.staged_state = new_state;
    //     Ok(())
    // }
}

// ======================================
//   HOLONBEHAVIOR TRAIT IMPLEMENTATIONS
// ======================================

impl ReadableHolonState for StagedHolon {
    fn all_related_holons(&self) -> Result<RelationshipMap, HolonError> {
        let relationship_map = RelationshipMap::from(self.get_staged_relationship_map()?);
        Ok(relationship_map)
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(self.property_map.clone(), self.key()?, self.errors.clone()))
    }

    fn holon_clone_model(&self) -> HolonCloneModel {
        HolonCloneModel::new(
            self.version.clone(),
            self.original_id.clone(),
            self.property_map.clone(),
            Some(RelationshipMap::from(self.staged_relationships.clone())),
        )
    }

    fn holon_id(&self) -> Result<HolonId, HolonError> {
        match &self.staged_state {
            StagedState::Abandoned => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
            StagedState::Committed(local_id) => Ok(HolonId::Local(local_id.clone())),
            StagedState::ForCreate => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
            StagedState::ForUpdate => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
            StagedState::ForUpdateChanged => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
        }
    }

    /// Retrieves the Holon's primary key, if defined in its `property_map`.
    fn key(&self) -> Result<Option<MapString>, HolonError> {
        let key_property_name = CorePropertyTypeName::Key.as_property_name();

        if let Some(BaseValue::StringValue(s)) = self.property_map.get(&key_property_name) {
            Ok(Some(s.clone()))
        } else {
            Ok(None)
        }
    }

    fn original_id(&self) -> Option<LocalId> {
        self.original_id.clone()
    }

    fn property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        Ok(self.property_map.get(property_name).cloned())
    }

    fn related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        Ok(self.staged_relationships.get_related_holons(relationship_name))
    }

    fn versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .key()?
            .ok_or(HolonError::InvalidParameter("StagedHolon must have a key".to_string()))?;

        Ok(MapString(format!("{}__{}_staged", key.0, &self.version.0.to_string())))
    }

    fn into_node_model(&self) -> HolonNodeModel {
        HolonNodeModel {
            original_id: self.original_id.clone(),
            property_map: self.property_map.clone(),
        }
    }

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
                        access_type.to_string(),
                        self.staged_state.to_string(),
                    )),
                },
            },
            HolonState::Immutable => match access_type {
                AccessType::Read | AccessType::Clone | AccessType::Commit => Ok(()),
                AccessType::Abandon | AccessType::Write => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    "Immutable".to_string(),
                )),
            },
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
            Err(_) => "<None>".to_string(),       // If local_id is not found or error occurred
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
            key, local_id, self.holon_state, self.validation_state
        )
    }
}

impl WriteableHolonState for StagedHolon {
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        self.staged_relationships.add_related_holons(context, relationship_name, holons)?;

        Ok(self)
    }

    fn increment_version(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.version.0 += 1;
        Ok(())
    }

    fn mark_as_immutable(&mut self) -> Result<(), HolonError> {
        self.holon_state = HolonState::Immutable;
        Ok(())
    }

    fn remove_property_value(&mut self, name: &PropertyName) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map.remove(name);
        Ok(self)
    }

    fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.staged_relationships.remove_related_holons(context, &relationship_name, holons)?;

        Ok(self)
    }

    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.original_id = id;

        Ok(())
    }

    fn with_property_value(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map.insert(property, value);

        Ok(self)
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
            property_map: BTreeMap::new(),
            staged_relationships: StagedRelationshipMap::new_empty(),
            original_id: None,
            errors: Vec::new(),
        };
        assert_eq!(initial_holon, expected_holon);

        // Create expected PropertyMap
        let mut expected_property_map = BTreeMap::new();
        let string_property_name = PropertyName(MapString("string property".to_string()));
        let string_value = BaseValue::StringValue(MapString("string_value".to_string()));
        expected_property_map.insert(string_property_name.clone(), string_value.clone());
        let boolean_property_name = PropertyName(MapString("boolean property".to_string()));
        let boolean_value = BaseValue::BooleanValue(MapBoolean(true));
        expected_property_map.insert(boolean_property_name.clone(), boolean_value.clone());
        let integer_property_name = PropertyName(MapString("integer property".to_string()));
        let integer_value = BaseValue::IntegerValue(MapInteger(1000));
        expected_property_map.insert(integer_property_name.clone(), integer_value.clone());
        let enum_property_name = PropertyName(MapString("enum property".to_string()));
        let enum_value = BaseValue::EnumValue(MapEnumValue(MapString("enum_value".to_string())));
        expected_property_map.insert(enum_property_name.clone(), enum_value.clone());

        // Add properties to Holon
        let _ = initial_holon
            .with_property_value(string_property_name, string_value)
            .unwrap()
            .with_property_value(boolean_property_name, boolean_value)
            .unwrap()
            .with_property_value(integer_property_name, integer_value)
            .unwrap()
            .with_property_value(enum_property_name, enum_value);

        assert_eq!(initial_holon.property_map, expected_property_map);
    }

    #[test]
    fn property_management() {
        let mut holon = StagedHolon::new_for_create();
        let property_name = PropertyName(MapString("first property".to_string()));
        // Add a value to the property map
        let initial_value = BaseValue::IntegerValue(MapInteger(1));
        holon.with_property_value(property_name.clone(), initial_value.clone()).unwrap();
        assert_eq!(holon.property_value(&property_name).unwrap(), Some(initial_value));
        // Update value for the same property name
        let changed_value = BaseValue::StringValue(MapString("changed value".to_string()));
        holon.with_property_value(property_name.clone(), changed_value.clone()).unwrap();
        assert_eq!(holon.property_value(&property_name).unwrap(), Some(changed_value));
        // Remove value by updating to None
        holon.remove_property_value(&property_name).unwrap();
        assert_eq!(holon.property_value(&property_name).unwrap(), None);
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
            property_map: BTreeMap::new(),
            staged_relationships: StagedRelationshipMap { map: BTreeMap::new() },
            original_id: None,
            errors: Vec::new(),
        };

        assert_eq!(default_holon, expected_holon);
    }
}
