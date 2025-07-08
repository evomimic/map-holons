// use crate::holon::behavior::HolonBehavior;
// use crate::common::{PropertyName, PropertyValue, EssentialHolonContent, MapString};
// use crate::HolonError, HolonState};
// use crate::state::AccessType;
// use crate::identifier::TemporaryId;

use serde::{Deserialize, Serialize};
use std::rc::Rc;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, TemporaryId};
use integrity_core_types::{HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue};

use crate::{
    core_shared_objects::{holon::StagedHolon, TransientRelationshipMap},
    HolonCollection, RelationshipName,
};

use super::{
    state::{AccessType, HolonState, ValidationState},
    EssentialHolonContent, HolonBehavior,
};

/// Represents a Holon that exists only in-memory and cannot be persisted unless it becomes a StagedHolon.
/// (for more information, see Holon LifeCycle [insert link] documentation/diagram)
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct TransientHolon {
    version: MapInteger,     // Used to add to hash content for creating TemporaryID
    holon_state: HolonState, // Mutable or Immutable
    validation_state: ValidationState,
    temporary_id: Option<TemporaryId>, // Ephemeral identifier for TransientHolons
    property_map: PropertyMap,         // Self-describing property data
    transient_relationships: TransientRelationshipMap, // Tracks ephemeral relationships
    original_id: Option<LocalId>,      // Tracks the predecessor, if cloned from a SavedHolon
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
            transient_relationships: TransientRelationshipMap::new_empty(),
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
            transient_relationships: TransientRelationshipMap::new_empty(),
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

    /// Converts the `TransientHolon` into a 'StagedHolon,
    ///
    /// This lifecycle transition takes place during the staging process when stage_holon is called by the nursery to update its staged pool.
    ///
    pub fn to_staged(self) -> Result<StagedHolon, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let mut staged_holon = StagedHolon::new_for_create();
        staged_holon.update_original_id(self.original_id.clone())?;
        staged_holon.update_property_map(self.property_map.clone())?;
        let map = self.get_transient_relationship_map()?;
        let staged_map = map.to_staged()?;
        staged_holon.init_relationships(staged_map)?;

        Ok(staged_holon)
    }

    pub fn update_relationship_map(
        &mut self,
        map: TransientRelationshipMap,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.transient_relationships = map;
        Ok(())
    }

    pub fn with_property_value(
        &mut self,
        property: PropertyName,
        value: Option<BaseValue>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map.insert(property, value);

        Ok(self)
    }

    // =====================
    //    DATA ACCESSORS
    // =====================

    pub fn get_related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Use the public `get_related_holons` method on the `TransientRelationshipMap`
        Ok(self.transient_relationships.get_related_holons(relationship_name))
    }

    pub fn get_transient_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.transient_relationships.get_related_holons(relationship_name))
    }

    pub fn get_transient_relationship_map(&self) -> Result<TransientRelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.transient_relationships.clone())
    }
}

// ======================================
//   HOLONBEHAVIOR TRAIT IMPLEMENTATION
// ======================================
impl HolonBehavior for TransientHolon {
    // ====================
    //    DATA ACCESSORS
    // ====================

    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        let mut holon = TransientHolon::new();

        // Copy the existing holon's PropertyMap into the new Holon
        holon.property_map = self.property_map.clone();

        // Update in place each relationship's HolonCollection State to Staged
        holon.transient_relationships = self.transient_relationships.clone_for_new_source()?;

        Ok(holon)
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        Ok(EssentialHolonContent::new(
            self.property_map.clone(),
            self.get_key()?,
            Vec::new(), // defaulting to empty
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

    fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .get_key()?
            .ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

        Ok(MapString(key.0 + &self.version.0.to_string()))
    }

    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        Err(HolonError::EmptyField("TransientHolons do not have LocalIds.".to_string()))
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

    fn into_node(&self) -> HolonNodeModel {
        HolonNodeModel::new(self.original_id.clone(), self.property_map.clone())
    }

    // =========================
    //     ACCESS CONTROL
    // =========================

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.holon_state {
            HolonState::Mutable => match access_type {
                AccessType::Read | AccessType::Write | AccessType::Clone | AccessType::Abandon => {
                    Ok(())
                }
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

    fn increment_version(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
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

    // ===================
    //       HELPERS
    // ===================

    fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.get_key() {
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

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use base_types::{MapBoolean, MapEnumValue};

    use super::*;

    #[test]
    fn instantiate_and_modify() {
        // Initialize default Holon
        let mut initial_holon = TransientHolon::new();
        let expected_holon = TransientHolon {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: BTreeMap::new(),
            transient_relationships: TransientRelationshipMap { map: BTreeMap::new() },
            original_id: None,
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
        let mut holon = TransientHolon::new();
        let property_name = PropertyName(MapString("first property".to_string()));
        // Add a value to the property map
        let initial_value = BaseValue::IntegerValue(MapInteger(1));
        holon.with_property_value(property_name.clone(), Some(initial_value.clone())).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), Some(initial_value));
        // Update value with the same property name
        let changed_value = BaseValue::StringValue(MapString("changed value".to_string()));
        holon.with_property_value(property_name.clone(), Some(changed_value.clone())).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), Some(changed_value));
        // Remove value by updating to None
        holon.with_property_value(property_name.clone(), None).unwrap();
        assert_eq!(holon.get_property_value(&property_name).unwrap(), None);
    }

    // #[test]
    // fn relationship_management() {
    //     let mut holon = TransientHolon::new();
    //     let relationship_name = RelationshipName(MapString("first relationship".to_string()));
    //     // let first_collection = HolonCollection::new_existing();
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

    // #[test]
    // fn () {

    // }

    #[test]
    fn try_modify_immutable_transient_holon() {
        let mut holon = TransientHolon::new_immutable();

        assert_eq!(
            holon.update_relationship_map(TransientRelationshipMap::new_empty()),
            Err(HolonError::NotAccessible(
                format!("{:?}", AccessType::Write),
                "Immutable TransientHolon".to_string(),
            ))
        );
        assert_eq!(
            holon.update_original_id(None),
            Err(HolonError::NotAccessible(
                format!("{:?}", AccessType::Write),
                "Immutable TransientHolon".to_string(),
            ))
        );
    }

    #[test]
    fn verify_default_values() {
        let default_holon = TransientHolon::new();
        let expected_holon = TransientHolon {
            version: MapInteger(1),
            holon_state: HolonState::Mutable,
            validation_state: ValidationState::ValidationRequired,
            temporary_id: None,
            property_map: BTreeMap::new(),
            transient_relationships: TransientRelationshipMap::new_empty(),
            original_id: None,
        };

        assert_eq!(default_holon, expected_holon);
    }
}
