// use crate::holon::behavior::HolonBehavior;
// use crate::common::{PropertyName, PropertyValue, EssentialHolonContent, MapString};
// use crate::HolonError, HolonState};
// use crate::state::AccessType;
// use crate::identifier::TemporaryId;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

use crate::{
    core_shared_objects::{
        holon::HolonCloneModel, holon_behavior::ReadableHolonState, TransientRelationshipMap,
        WriteableHolonState,
    },
    HolonCollection, HolonReference, HolonsContextBehavior, RelationshipMap,
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue,
    RelationshipName,
};
use type_names::CorePropertyTypeName;

use super::{
    state::{AccessType, HolonState, ValidationState},
    EssentialHolonContent,
};

/// Represents a Holon that exists only in-memory and cannot be persisted unless it becomes a StagedHolon.
/// (for more information, see Holon LifeCycle [insert link] documentation/diagram)
///
/// Default fields:
/// holon_state: HolonState::Mutable,
/// validation_state: ValidationState::ValidationRequired,
/// property_map: PropertyMap::
/// transient_relationships: TransientRelationshipMap::new_empty(),
/// original_id: None,
///
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientHolon {
    version: MapInteger,     // Used to add to hash content for creating TemporaryID
    holon_state: HolonState, // Mutable or Immutable
    validation_state: ValidationState,
    property_map: PropertyMap, // Self-describing property data
    transient_relationships: TransientRelationshipMap, // Tracks ephemeral relationships
    original_id: Option<LocalId>, // Tracks the predecessor, if cloned from a SavedHolon
}

// ==================================
//   ASSOCIATED METHODS (IMPL BLOCK)
// ==================================
impl TransientHolon {
    // Note: Constructors delegated via TransientHolonManager

    pub(crate) fn with_fields(
        version: MapInteger,
        holon_state: HolonState,
        validation_state: ValidationState,
        property_map: PropertyMap,
        transient_relationships: TransientRelationshipMap,
        original_id: Option<LocalId>,
    ) -> Self {
        Self {
            version,
            holon_state,
            validation_state,
            property_map,
            transient_relationships,
            original_id,
        }
    }

    /// Retrieves a transient relationship after verifying read access
    pub fn get_transient_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.transient_relationships.get_related_holons(relationship_name))
    }

    fn get_transient_relationship_map(&self) -> Result<TransientRelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;

        Ok(self.transient_relationships.clone())
    }

    /// Returns a cloned snapshot of the raw property map.
    /// Kept crate-visible; caller should enforce appropriate access checks.
    pub(crate) fn raw_property_map_clone(&self) -> PropertyMap {
        self.property_map.clone()
    }
}

// ======================================
//   HOLONBEHAVIOR TRAIT IMPLEMENTATIONS
// ======================================

impl ReadableHolonState for TransientHolon {
    fn all_related_holons(&self) -> Result<RelationshipMap, HolonError> {
        let relationship_map = RelationshipMap::from(self.get_transient_relationship_map()?);

        Ok(relationship_map)
    }

    fn essential_content(&self) -> EssentialHolonContent {
        EssentialHolonContent::new(
            self.property_map.clone(),
            self.key(),
            Vec::new(), // defaulting to empty
        )
    }

    fn holon_clone_model(&self) -> HolonCloneModel {
        HolonCloneModel::new(
            self.version.clone(),
            self.original_id.clone(),
            self.property_map.clone(),
            Some(RelationshipMap::from(self.transient_relationships.clone())),
        )
    }

    fn holon_id(&self) -> Result<HolonId, HolonError> {
        Err(HolonError::NotImplemented("TransientHolons do not have a HolonId".to_string()))
    }

    fn key(&self) -> Option<MapString> {
        // Use canonical PascalCase property name
        let key_property_name = CorePropertyTypeName::Key.as_property_name();

        if let Some(BaseValue::StringValue(s)) = self.property_map.get(&key_property_name) {
            Some(s.clone())
        } else {
            None
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
        Ok(self.transient_relationships.get_related_holons(relationship_name))
    }

    fn versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .key()
            .ok_or(HolonError::InvalidParameter("TransientHolon must have a key".to_string()))?;

        Ok(MapString(format!("{}__{}_transient", key.0, &self.version.0.to_string())))
    }

    fn into_node_model(&self) -> HolonNodeModel {
        HolonNodeModel::new(self.original_id.clone(), self.property_map.clone())
    }

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

    fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.key() {
            Some(key) => key.0,           // Extract the key from MapString
            None => "<None>".to_string(), // Key is None
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, state: {}, validation_state: {:?} }}",
            key, self.holon_state, self.validation_state
        )
    }
}

impl WriteableHolonState for TransientHolon {
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        self.transient_relationships.add_related_holons(context, relationship_name, holons)?;

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
        self.transient_relationships.remove_related_holons(context, &relationship_name, holons)?;

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

// TODO: fix or delete
// #[cfg(test)]
// mod tests {

//     use std::collections::BTreeMap;

//     use base_types::{MapBoolean, MapEnumValue};

//     use crate::{
//         core_shared_objects::TransientHolonManager, reference_layer::TransientHolonBehavior,
//     };

//     use super::*;
//
//     #[test]
//     fn instantiate_and_modify() {
//         let transient_manager = TransientHolonManager::new();

//         let mut initial_holon = transient_manager.create_empty().unwrap();
//         let expected_holon = TransientHolon {
//             version: MapInteger(1),
//             holon_state: HolonState::Mutable,
//             validation_state: ValidationState::ValidationRequired,
//             property_map: BTreeMap::new(),
//             transient_relationships: TransientRelationshipMap { map: BTreeMap::new() },
//             original_id: None,
//         };

//         assert_eq!(initial_holon, expected_holon);

//         // Create example PropertyMap and modify Holon with its update
//         let mut property_map = BTreeMap::new();
//         let string_property_name = PropertyName(MapString("string property".to_string()));
//         let string_value = BaseValue::StringValue(MapString("string_value".to_string()));
//         property_map.insert(string_property_name, string_value);
//         let boolean_property_name = PropertyName(MapString("boolean property".to_string()));
//         let boolean_value = BaseValue::BooleanValue(MapBoolean(true));
//         property_map.insert(boolean_property_name, boolean_value);
//         let integer_property_name = PropertyName(MapString("integer property".to_string()));
//         let integer_value = BaseValue::IntegerValue(MapInteger(1000));
//         property_map.insert(integer_property_name, integer_value);
//         let enum_property_name = PropertyName(MapString("enum property".to_string()));
//         let enum_value = BaseValue::EnumValue(MapEnumValue(MapString("enum_value".to_string())));
//         property_map.insert(enum_property_name, enum_value);

//         initial_holon.update_property_map(property_map.clone()).unwrap();

//         assert_eq!(initial_holon.property_map, property_map);
//     }

//     #[test]
//     fn property_management() {
//         let transient_manager = TransientHolonManager::new();

//         let mut holon = transient_manager.create_empty().unwrap();
//         let property_name = PropertyName(MapString("first property".to_string()));

//         // Add a value to the property map
//         let initial_value = BaseValue::IntegerValue(MapInteger(1));
//         holon.with_property_value(property_name.clone(), initial_value.clone()).unwrap();
//         assert_eq!(holon.property_value(&property_name).unwrap(), Some(initial_value));

//         // Update value with the same property name
//         let changed_value = BaseValue::StringValue(MapString("changed value".to_string()));
//         holon.with_property_value(property_name.clone(), changed_value.clone()).unwrap();
//         assert_eq!(holon.property_value(&property_name).unwrap(), Some(changed_value));

//         // Remove value by updating to None
//         holon.remove_property_value(&property_name).unwrap();
//         assert_eq!(holon.property_value(&property_name).unwrap(), None);
//     }

//     // #[test]
//     // fn relationship_management() {
//     //     let mut holon = TransientHolon::new();
//     //     let relationship_name = RelationshipName(MapString("first relationship".to_string()));
//     //     // let first_collection = HolonCollection::new_existing();
//     //     //

//     //     // // Add a relationship to the relationship map
//     //     // holon.add_related_holon(relationship_name.clone(), Some(initial_value));
//     //     // assert_eq!();
//     //     // // Update relationship value for the same relationship name
//     //     // let changed_collection = BaseValue::StringValue(MapString("changed value".to_string()));
//     //     // holon.remove_related_holon();
//     //     // assert_eq!();

//     //     // TODO: remove relationship
//     // }

//     // #[test]
//     // fn () {

//     // }

//     #[test]
//     fn try_modify_immutable_transient_holon() {
//         let transient_manager = TransientHolonManager::new();
//         let mut holon = transient_manager.create_immutable().unwrap();

//         assert_eq!(
//             holon.update_relationship_map(TransientRelationshipMap::new_empty()),
//             Err(HolonError::NotAccessible(
//                 format!("{:?}", AccessType::Write),
//                 "Immutable TransientHolon".to_string(),
//             ))
//         );
//         assert_eq!(
//             holon.update_original_id(None),
//             Err(HolonError::NotAccessible(
//                 format!("{:?}", AccessType::Write),
//                 "Immutable TransientHolon".to_string(),
//             ))
//         );
//     }

//     #[test]
//     fn verify_default_values() {
//         let default_holon = TransientHolon::new();
//         let expected_holon = TransientHolon {
//             version: MapInteger(1),
//             holon_state: HolonState::Mutable,
//             validation_state: ValidationState::ValidationRequired,
//             property_map: BTreeMap::new(),
//             transient_relationships: TransientRelationshipMap::new_empty(),
//             original_id: None,
//         };

//         assert_eq!(default_holon, expected_holon);
//     }
// }
