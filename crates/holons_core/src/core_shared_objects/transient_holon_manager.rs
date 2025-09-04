use std::{any::Any, cell::RefCell, rc::Rc};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, TemporaryId};
use integrity_core_types::{PropertyMap, PropertyName};

use crate::{
    core_shared_objects::{
        holon::{
            state::{HolonState, ValidationState},
            Holon, HolonCloneModel, TransientHolon,
        },
        holon_pool::{SerializableHolonPool, TransientHolonPool},
        transient_manager_access_internal::TransientManagerAccessInternal,
        ReadableRelationship, TransientManagerAccess, TransientRelationshipMap,
    },
    reference_layer::{TransientHolonBehavior, TransientReference},
    HolonPool, HolonsContextBehavior,
};

/// Holon variant-agnostic interface for cloning.
///
/// Regardless of the source phase, cloned Holons always begin their lifecycle as `TransientHolon`.
pub trait ToHolonCloneModel {
    fn get_holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransientHolonManager {
    transient_holons: Rc<RefCell<TransientHolonPool>>, // Uses Rc<RefCell<HolonPool>> for interior mutability
}

impl TransientHolonManager {
    pub fn new_empty() -> Self {
        Self { transient_holons: Rc::new(RefCell::new(TransientHolonPool(HolonPool::new()))) }
    }

    pub fn new_with_pool(pool: TransientHolonPool) -> Self {
        Self { transient_holons: Rc::new(RefCell::new(pool)) }
    }

    /// Adds the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the transient holon
    /// to be retrieved by key.
    fn add_new_holon(&self, holon: TransientHolon) -> Result<TransientReference, HolonError> {
        let id = self.transient_holons.borrow_mut().insert_holon(Holon::Transient(holon))?;
        self.to_validated_transient_reference(&id)
    }

    /// This function converts a TemporaryId into a TransientReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_transient_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<TransientReference, HolonError> {
        // Determine if the id references a TransientHolon in the transient manager
        let _holon_rc = self.get_holon_by_id(id)?;

        Ok(TransientReference::from_temporary_id(id))
    }
}

impl TransientManagerAccess for TransientHolonManager {
    /// Retrieves a transient holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.transient_holons.borrow().get_holon_by_id(id)
    }
}

impl TransientHolonBehavior for TransientHolonManager {
    fn create_empty(&self, key: MapString) -> Result<TransientReference, HolonError> {
        let mut property_map = PropertyMap::new();
        property_map
            .insert(PropertyName(MapString("key".to_string())), BaseValue::StringValue(key));
        let holon = TransientHolon::with_fields(
            MapInteger(1),
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            // None,
            property_map,
            TransientRelationshipMap::new_empty(),
            None,
        );
        let transient_reference = self.add_new_holon(holon)?;

        Ok(transient_reference)
    }

    fn new_from_clone_model(
        &self,
        holon_clone_model: HolonCloneModel,
    ) -> Result<TransientReference, HolonError> {
        let transient_relationships = {
            if let Some(relationship_map) = holon_clone_model.relationships {
                relationship_map.clone_for_new_source()?
            } else {
                return Err(HolonError::InvalidParameter("HolonCloneModel passed through this constructor must always contain a RelationshipMap, even if empty".to_string()));
            }
        };
        let holon = TransientHolon::with_fields(
            holon_clone_model.version,
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            // None,
            holon_clone_model.properties,
            transient_relationships,
            holon_clone_model.original_id,
        );

        let transient_reference = self.add_new_holon(holon)?;

        Ok(transient_reference)
    }

    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_transient_holon_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = self.transient_holons.borrow().get_id_by_base_key(key)?;
        self.to_validated_transient_reference(&id)
    }

    fn get_transient_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<TransientReference>, HolonError> {
        let mut transient_references = Vec::new();
        let transient_manager = self.transient_holons.borrow();
        let ids = transient_manager.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_transient_reference = self.to_validated_transient_reference(&id)?;
            transient_references.push(validated_transient_reference);
        }

        Ok(transient_references)
    }

    fn get_transient_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = self.transient_holons.borrow().get_id_by_versioned_key(key)?;
        self.to_validated_transient_reference(&id)
    }

    fn transient_count(&self) -> i64 {
        self.transient_holons.borrow().len() as i64
    }
}

impl TransientManagerAccessInternal for TransientHolonManager {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear_pool(&mut self) {
        self.transient_holons.borrow_mut().clear();
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.transient_holons.borrow().get_id_by_versioned_key(key)
    }

    fn get_transient_holons_pool(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.transient_holons.borrow().get_all_holons()
    }

    fn export_transient_holons(&self) -> SerializableHolonPool {
        self.transient_holons.borrow().export_pool()
    }

    fn import_transient_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.transient_holons.borrow_mut().import_pool(pool); // Mutates existing HolonPool
    }
}
