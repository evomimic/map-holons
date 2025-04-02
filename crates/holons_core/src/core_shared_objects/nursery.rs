use crate::core_shared_objects::holon_pool::{HolonPool, SerializableHolonPool};
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::{Holon, HolonError, HolonState, NurseryAccess};
use crate::reference_layer::{HolonStagingBehavior, StagedReference};
use shared_types_holon::holon_node::TemporaryId;

use shared_types_holon::MapString;
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
    fn stage_holon(&self, holon: Holon) -> Result<TemporaryId, HolonError> {
        self.staged_holons.borrow_mut().insert_holon(holon)
    }

    /// This function converts a TemporaryId into a StagedReference
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    fn to_validated_staged_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<StagedReference, HolonError> {
        let holon_rc = self.get_holon_by_id(id)?;

        let holon = holon_rc.borrow();
        if let HolonState::Abandoned = holon.state {
            return Err(HolonError::NotAccessible(
                "to_validated_staged_reference".to_string(),
                "Abandoned".to_string(),
            ));
        }

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
    fn get_staged_holon_by_key(&self, key: &MapString) -> Result<StagedReference, HolonError> {
        let id = self.staged_holons.borrow().get_id_by_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn staged_count(&self) -> i64 {
        self.staged_holons.borrow().len() as i64
    }

    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError> {
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

    fn get_id_by_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.staged_holons.borrow().get_id_by_key(key)
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
