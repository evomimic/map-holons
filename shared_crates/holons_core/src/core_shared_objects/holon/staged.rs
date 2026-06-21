use std::sync::{Arc, RwLock};

use crate::{
    core_shared_objects::{
        holon::{
            AccessType, EssentialHolonContent, HolonCloneModel, HolonState, StagedState,
            ValidationState,
        },
        ReadableHolonState, ReadableRelationship, WritableRelationship, WriteableHolonState,
    },
    HolonCollection, HolonReference, RelationshipMap, StagedRelationshipMap,
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue,
    RelationshipName,
};
use type_names::CorePropertyTypeName;

/// Represents a Holon that has been staged for persistence or updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedHolon {
    version: MapInteger,       // Used to add to hash content for creating TemporaryID
    holon_state: HolonState,   // Mutable or Immutable
    staged_state: StagedState, // ForCreate, update states, Abandoned, or Committed
    validation_state: ValidationState,
    property_map: PropertyMap, // Self-describing property data
    staged_relationships: StagedRelationshipMap,
    // Holochain lineage root for the node model. This is distinct from MAP version lineage.
    original_id: Option<LocalId>,
    // Current persisted node being staged for update; graph-only commits anchor relationships here.
    versioned_source_id: Option<LocalId>,
    errors: Vec<HolonError>, // Populated during the commit process
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
            versioned_source_id: None,
            errors: Vec::new(),
        }
    }

    /// Creates a new StagedHolon in the `ForCreate` state.   
    pub fn new_from_clone_model(model: HolonCloneModel) -> Result<Self, HolonError> {
        let staged_relationships = Self::staged_relationships_from_model(&model)?;
        let staged_holon = Self {
            version: model.version,
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForCreate,
            validation_state: ValidationState::ValidationRequired,
            property_map: model.properties,
            staged_relationships,
            original_id: model.original_id,
            versioned_source_id: None,
            errors: Vec::new(),
        };

        Ok(staged_holon)
    }

    /// Creates a content-preserving staged update for an existing persisted Holon.
    pub fn new_for_update_from_clone_model(
        model: HolonCloneModel,
        source_local_id: LocalId,
    ) -> Result<Self, HolonError> {
        let staged_relationships = Self::staged_relationships_from_model(&model)?;

        Ok(Self {
            version: model.version,
            holon_state: HolonState::Mutable,
            staged_state: StagedState::ForUpdate,
            validation_state: ValidationState::ValidationRequired,
            property_map: model.properties,
            staged_relationships,
            original_id: model.original_id,
            versioned_source_id: Some(source_local_id),
            errors: Vec::new(),
        })
    }

    /// Creates a staged holon from pre-validated constituent parts.
    pub fn from_parts(
        version: MapInteger,
        holon_state: HolonState,
        staged_state: StagedState,
        validation_state: ValidationState,
        property_map: PropertyMap,
        staged_relationships: StagedRelationshipMap,
        original_id: Option<LocalId>,
        versioned_source_id: Option<LocalId>,
        errors: Vec<HolonError>,
    ) -> Self {
        Self {
            version,
            holon_state,
            staged_state,
            validation_state,
            property_map,
            staged_relationships,
            original_id,
            versioned_source_id,
            errors,
        }
    }

    fn staged_relationships_from_model(
        model: &HolonCloneModel,
    ) -> Result<StagedRelationshipMap, HolonError> {
        if let Some(relationship_map) = &model.relationships {
            relationship_map.clone_for_staged() // Skips any TransientReference members
        } else {
            Err(HolonError::InvalidParameter("HolonCloneModel passed through this constructor must always contain a RelationshipMap, even if empty".to_string()))
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

    pub fn version(&self) -> &MapInteger {
        &self.version
    }

    pub fn holon_state(&self) -> &HolonState {
        &self.holon_state
    }

    pub fn staged_state(&self) -> &StagedState {
        &self.staged_state
    }

    pub fn validation_state(&self) -> &ValidationState {
        &self.validation_state
    }

    pub fn property_map(&self) -> &PropertyMap {
        &self.property_map
    }

    pub fn staged_relationships(&self) -> &StagedRelationshipMap {
        &self.staged_relationships
    }

    pub fn original_id_ref(&self) -> Option<&LocalId> {
        self.original_id.as_ref()
    }

    pub fn get_versioned_source_id(&self) -> Result<LocalId, HolonError> {
        self.versioned_source_id.clone().ok_or(HolonError::EmptyField(
            "StagedHolon update is missing its persisted source LocalId.".to_string(),
        ))
    }

    pub fn versioned_source_id_ref(&self) -> Option<&LocalId> {
        self.versioned_source_id.as_ref()
    }

    pub fn errors(&self) -> &[HolonError] {
        &self.errors
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
            StagedState::ForCreate
            | StagedState::ForUpdate
            | StagedState::ForUpdateGraphOnly
            | StagedState::ForUpdateNewVersion => {
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

    /// Notes a content mutation and escalates existing-holon updates to a new version.
    pub fn note_property_mutation(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        match self.staged_state {
            StagedState::ForUpdate | StagedState::ForUpdateGraphOnly => {
                self.staged_state = StagedState::ForUpdateNewVersion;
            }
            StagedState::ForCreate | StagedState::ForUpdateNewVersion => {}
            StagedState::Abandoned | StagedState::Committed(_) => {
                return Err(HolonError::NotAccessible(
                    AccessType::Write.to_string(),
                    self.staged_state.to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Notes a relationship mutation after descriptor-backed definitional classification.
    pub fn note_relationship_mutation(&mut self, is_definitional: bool) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        match (&self.staged_state, is_definitional) {
            (StagedState::ForUpdate, false) => {
                self.staged_state = StagedState::ForUpdateGraphOnly;
            }
            (StagedState::ForUpdate | StagedState::ForUpdateGraphOnly, true) => {
                self.staged_state = StagedState::ForUpdateNewVersion;
            }
            (
                StagedState::ForCreate
                | StagedState::ForUpdateGraphOnly
                | StagedState::ForUpdateNewVersion,
                false,
            )
            | (StagedState::ForCreate | StagedState::ForUpdateNewVersion, true) => {}
            (StagedState::Abandoned | StagedState::Committed(_), _) => {
                return Err(HolonError::NotAccessible(
                    AccessType::Write.to_string(),
                    self.staged_state.to_string(),
                ));
            }
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

    fn essential_content(&self) -> EssentialHolonContent {
        EssentialHolonContent::new(self.property_map.clone(), self.key(), self.errors.clone())
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
            StagedState::ForUpdateGraphOnly => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
            StagedState::ForUpdateNewVersion => Err(HolonError::NotImplemented(
                "StagedHolons only have a HolonId if they are Saved (in a StagedState::Committed)"
                    .to_string(),
            )),
        }
    }

    fn into_node_model(&self) -> HolonNodeModel {
        HolonNodeModel {
            original_id: self.original_id.clone(),
            property_map: self.property_map.clone(),
        }
    }

    /// Retrieves the Holon's primary key, if defined in its `property_map`.
    fn key(&self) -> Option<MapString> {
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
        Ok(self.staged_relationships.get_related_holons(relationship_name))
    }

    fn versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .key()
            .ok_or(HolonError::InvalidParameter("StagedHolon must have a key".to_string()))?;

        Ok(MapString(format!("{}__{}_staged", key.0, &self.version.0.to_string())))
    }

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.holon_state {
            HolonState::Mutable => match self.staged_state {
                StagedState::ForCreate
                | StagedState::ForUpdate
                | StagedState::ForUpdateGraphOnly
                | StagedState::ForUpdateNewVersion => match access_type {
                    AccessType::Read
                    | AccessType::Write
                    | AccessType::Clone
                    | AccessType::Abandon
                    | AccessType::Commit => Ok(()),
                },
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
            Some(key) => key.0,           // Extract the key from MapString
            None => "<None>".to_string(), // Key is None
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
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        self.staged_relationships.add_related_holons(relationship_name, holons)?;

        Ok(self)
    }

    /// Adds related holons using precomputed keys to avoid key lookups while the holon is locked.
    fn add_related_holons_with_keys(
        &mut self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.staged_relationships.add_related_holons_with_keys(relationship_name, entries)?;
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
        self.note_property_mutation()?;
        Ok(self)
    }

    fn remove_related_holons(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.staged_relationships.remove_related_holons(&relationship_name, holons)?;

        Ok(self)
    }

    /// Removes related holons using precomputed keys to avoid key lookups while the holon is locked.
    fn remove_related_holons_with_keys(
        &mut self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.staged_relationships.remove_related_holons_with_keys(relationship_name, entries)?;
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
        self.note_property_mutation()?;

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
            versioned_source_id: None,
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
            versioned_source_id: None,
            errors: Vec::new(),
        };

        assert_eq!(default_holon, expected_holon);
    }

    #[test]
    fn new_for_update_from_clone_model_preserves_content_and_source_anchor() {
        let source_id = LocalId(vec![1, 2, 3]);
        let original_id = LocalId(vec![4, 5, 6]);
        let property_name = PropertyName(MapString("name".to_string()));
        let property_value = BaseValue::StringValue(MapString("value".to_string()));
        let mut properties = PropertyMap::new();
        properties.insert(property_name.clone(), property_value.clone());
        let model = HolonCloneModel::new(
            MapInteger(7),
            Some(original_id.clone()),
            properties,
            Some(RelationshipMap::new_empty()),
        );

        let staged =
            StagedHolon::new_for_update_from_clone_model(model, source_id.clone()).unwrap();

        assert_eq!(staged.staged_state(), &StagedState::ForUpdate);
        assert_eq!(staged.version(), &MapInteger(7));
        assert_eq!(staged.original_id_ref(), Some(&original_id));
        assert_eq!(staged.versioned_source_id_ref(), Some(&source_id));
        assert_eq!(staged.property_value(&property_name).unwrap(), Some(property_value));
    }

    #[test]
    fn property_mutation_escalates_existing_updates_to_new_version() {
        let source_id = LocalId(vec![1, 2, 3]);
        let model = HolonCloneModel::new(
            MapInteger(1),
            None,
            PropertyMap::new(),
            Some(RelationshipMap::new_empty()),
        );
        let mut staged = StagedHolon::new_for_update_from_clone_model(model, source_id).unwrap();

        staged.note_property_mutation().unwrap();

        assert_eq!(staged.staged_state(), &StagedState::ForUpdateNewVersion);
    }

    #[test]
    fn relationship_mutation_tracks_graph_only_and_escalates_without_downgrade() {
        let source_id = LocalId(vec![1, 2, 3]);
        let model = HolonCloneModel::new(
            MapInteger(1),
            None,
            PropertyMap::new(),
            Some(RelationshipMap::new_empty()),
        );
        let mut staged = StagedHolon::new_for_update_from_clone_model(model, source_id).unwrap();

        staged.note_relationship_mutation(false).unwrap();
        assert_eq!(staged.staged_state(), &StagedState::ForUpdateGraphOnly);

        staged.note_relationship_mutation(true).unwrap();
        assert_eq!(staged.staged_state(), &StagedState::ForUpdateNewVersion);

        staged.note_relationship_mutation(false).unwrap();
        assert_eq!(staged.staged_state(), &StagedState::ForUpdateNewVersion);
    }
}
