use super::{
    holon_pool::{HolonPool, SerializableHolonPool},
    nursery_access_internal::NurseryAccessInternal,
    Holon, TransientHolon,
};
use crate::reference_layer::{HolonStagingBehavior, StagedReference};
use crate::{HolonError, NurseryAccess};
use base_types::MapString;
use core_types::TemporaryId;
use std::any::Any;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nursery {
    staged_holons: Rc<RefCell<HolonPool>>, // Uses Rc<RefCell<HolonPool>> for interior mutability
}

impl Nursery {
    /// Creates a new Nursery with an empty HolonPool
    pub fn new() -> Self {
        Self { staged_holons: Rc::new(RefCell::new(HolonPool::new())) }
    }

    // pub fn as_internal(&self) -> &dyn NurseryAccessInternal {
    //     self
    // }

    /// Stages a new holon.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The TemporaryId, which is used a unique identifier.
    fn stage_holon(&self, holon: TransientHolon) -> Result<TemporaryId, HolonError> {
        let staged_holon = holon.to_staged()?;
        self.staged_holons.borrow_mut().insert_holon(Holon::Staged(staged_holon))
    }

    /// This function converts a TemporaryId into a StagedReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_staged_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<StagedReference, HolonError> {
        // Determine if the id references a StagedHolon in the Nursery
        let _holon_rc = self.get_holon_by_id(id)?;

        Ok(StagedReference::from_temporary_id(id))
    }
}

impl NurseryAccess for Nursery {
    /// Retrieves a staged holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.staged_holons.borrow().get_holon_by_id(id)
    }
}

impl HolonStagingBehavior for Nursery {
    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_staged_holon_by_base_key(&self, key: &MapString) -> Result<StagedReference, HolonError> {
        let id = self.staged_holons.borrow().get_id_by_base_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn get_staged_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<StagedReference>, HolonError> {
        let mut staged_references = Vec::new();
        let nursery = self.staged_holons.borrow();
        let ids = nursery.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_staged_reference = self.to_validated_staged_reference(&id)?;
            staged_references.push(validated_staged_reference);
        }

        Ok(staged_references)
    }

    /// Does a lookup by full (unique) key on staged holons.
    fn get_staged_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<StagedReference, HolonError> {
        let id = self.staged_holons.borrow().get_id_by_versioned_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn staged_count(&self) -> i64 {
        self.staged_holons.borrow().len() as i64
    }

    fn stage_new_holon(&self, holon: TransientHolon) -> Result<StagedReference, HolonError> {
        let new_id = self.stage_holon(holon)?;
        self.to_validated_staged_reference(&new_id)
    }
}

impl NurseryAccessInternal for Nursery {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear_stage(&mut self) {
        self.staged_holons.borrow_mut().clear();
    }

    // fn get_keyed_index(&self) -> BTreeMap<MapString, usize> {
    //     self.holon_store.borrow().keyed_index.clone()
    // }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.staged_holons.borrow().get_id_by_versioned_key(key)
    }

    /// Exports the staged holons using `SerializableHolonPool`
    fn export_staged_holons(&self) -> SerializableHolonPool {
        self.staged_holons.borrow().export_pool()
    }

    fn import_staged_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.staged_holons.borrow_mut().import_pool(pool); // Mutates existing HolonPool
    }

    /// Returns the staged Holons in the `HolonPool`,
    /// ensuring that commit functions can access the actual Holon instances.
    // fn get_holons_to_commit(&self) -> impl Iterator<Item = Rc<RefCell<Holon>>> + '_ {
    fn get_holons_to_commit(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.staged_holons.borrow().get_all_holons()
    }

    // fn stage_holon(&self, holon: Holon) -> usize {
    //     self.staged_holons.borrow_mut().insert_holon(holon)
    // }
}
