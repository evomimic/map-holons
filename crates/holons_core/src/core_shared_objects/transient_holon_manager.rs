use std::{any::Any, cell::RefCell, rc::Rc};

use base_types::MapString;
use core_types::{HolonError, TemporaryId};

use crate::{
    core_shared_objects::{
        holon::{Holon, TransientHolon},
        holon_pool::{SerializableHolonPool, TransientHolonPool},
        transient_manager_access_internal::TransientManagerAccessInternal,
        TransientManagerAccess,
    },
    reference_layer::{TransientHolonBehavior, TransientReference},
    HolonPool,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransientHolonManager {
    transient_holons: Rc<RefCell<TransientHolonPool>>, // Uses Rc<RefCell<HolonPool>> for interior mutability
}

impl TransientHolonManager {
    pub fn new() -> Self {
        Self { transient_holons: Rc::new(RefCell::new(TransientHolonPool(HolonPool::new()))) }
    }

    /// Inserts a new holon.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon.
    ///
    /// # Returns
    /// The TemporaryId, which is used a unique identifier.
    fn add_holon(&self, holon: TransientHolon) -> Result<TemporaryId, HolonError> {
        self.transient_holons.borrow_mut().0.insert_holon(Holon::Transient(holon))
    }

    /// This function converts a TemporaryId into a TransientReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_transient_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<TransientReference, HolonError> {
        // Determine if the id references a TransientHolon in the Nursery
        let _holon_rc = self.get_holon_by_id(id)?;

        Ok(TransientReference::from_temporary_id(id))
    }
}

impl TransientManagerAccess for TransientHolonManager {
    /// Retrieves a transient holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.transient_holons.borrow().0.get_holon_by_id(id)
    }
}

impl TransientHolonBehavior for TransientHolonManager {
    fn add_new_holon(&self, holon: TransientHolon) -> Result<TransientReference, HolonError> {
        let new_id = self.add_holon(holon)?;
        self.to_validated_transient_reference(&new_id)
    }

    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_transient_holon_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = self.transient_holons.borrow().0.get_id_by_base_key(key)?;
        self.to_validated_transient_reference(&id)
    }

    fn get_transient_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<TransientReference>, HolonError> {
        let mut transient_references = Vec::new();
        let transient_manager = self.transient_holons.borrow();
        let ids = transient_manager.0.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_transient_reference = self.to_validated_transient_reference(&id)?;
            transient_references.push(validated_transient_reference);
        }

        Ok(transient_references)
    }

    /// Does a lookup by full (unique) key on transient holons.
    fn get_transient_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = self.transient_holons.borrow().0.get_id_by_versioned_key(key)?;
        self.to_validated_transient_reference(&id)
    }

    fn transient_count(&self) -> i64 {
        self.transient_holons.borrow().0.len() as i64
    }
}

impl TransientManagerAccessInternal for TransientHolonManager {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear_pool(&mut self) {
        self.transient_holons.borrow_mut().0.clear();
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.transient_holons.borrow().0.get_id_by_versioned_key(key)
    }

    fn get_transient_holons_pool(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.transient_holons.borrow().0.get_all_holons()
    }

    fn export_transient_holons(&self) -> SerializableHolonPool {
        self.transient_holons.borrow().0.export_pool()
    }

    fn import_transient_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.transient_holons.borrow_mut().0.import_pool(pool); // Mutates existing HolonPool
    }
}
