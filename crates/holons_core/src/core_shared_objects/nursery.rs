use crate::core_shared_objects::holon_pool::{HolonPool, SerializableHolonPool};
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::{Holon, HolonError, HolonState, NurseryAccess, RelationshipMap};
use crate::reference_layer::staged_reference::StagedIndex;
use crate::reference_layer::{
    HolonReference, HolonStagingBehavior, HolonsContextBehavior, SmartReference, StagedReference,
};

use shared_types_holon::{HolonId, MapString};
use std::any::Any;
use std::cell::Ref;
use std::{cell::RefCell, rc::Rc};
// #[hdk_entry_helper]
// #[derive(Clone, PartialEq, Eq)]
// pub struct Nursery {
//     staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
//     keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
// }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nursery {
    staged_holons: Rc<RefCell<HolonPool>>, // Uses Rc<RefCell<HolonPool>> for interior mutability
}

impl Nursery {
    /// Creates a new Nursery with an empty HolonPool
    pub fn new() -> Self {
        Self { staged_holons: Rc::new(RefCell::new(HolonPool::new())) }
    }
    // /// Initializes a `Nursery` from a set of staged holons and a keyed index
    // ///
    // /// # Arguments
    // ///
    // /// * `staged_holons` - A vector of staged holons.
    // /// * `keyed_index` - A map of keys to indices into the staged_holons vector
    // ///
    // /// # Returns
    // ///
    // /// A `Nursery` instance initialized with the provided holons and keyed index.
    // /// Creates a new Nursery from a given stage
    // /// Initializes a `Nursery` from a set of Holons and a keyed index.
    // pub fn new_from_staged_holons(
    //     holons: Vec<Rc<RefCell<Holon>>>,
    //     keyed_index: BTreeMap<MapString, usize>,
    // ) -> Self {
    //     Self { staged_holons: RefCell::new(HolonPool::new(holons, keyed_index)) }
    // }
    // pub fn as_internal(&self) -> &dyn NurseryAccessInternal {
    //     self
    // }
    fn clone_existing_relationships_into_staged_map(
        &self,
        _original_holon: HolonId,
        _staged_holon: &Holon,
    ) -> Result<Rc<RelationshipMap>, HolonError> {
        todo!()
    }

    /// Stages a new holon and optionally updates the keyed index.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The index of the staged holon in the nursery.
    fn stage_holon(&self, holon: Holon) -> usize {
        self.staged_holons.borrow_mut().insert_holon(holon)
    }

    /// This function converts a StagedIndex into a StagedReference, first validating accessibility
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    fn to_validated_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Ok(holon_rc) = self.get_holon_by_index(staged_index) {
            let holon = holon_rc.borrow();
            if let HolonState::Abandoned = holon.state {
                return Err(HolonError::NotAccessible(
                    "to_validated_staged_reference".to_string(),
                    "Abandoned".to_string(),
                ));
            }
            Ok(StagedReference::from_index(staged_index))
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }

    /// Checks if an index is valid within the `staged_holons` vector.
    ///
    /// # Arguments
    ///
    /// * `index` - The index to check.
    ///
    /// # Returns
    ///
    /// `true` if the index is valid, `false` otherwise.
    #[allow(dead_code)]
    pub fn is_valid_index(&self, index: usize) -> bool {
        self.staged_holons.borrow().is_valid_index(index)
    }
}
impl NurseryAccess for Nursery {
    /// Retrieves a staged holon by index.
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.staged_holons.borrow().get_holon_by_index(index)
    }
}

impl HolonStagingBehavior for Nursery {
    fn get_staged_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError> {
        let index = self.staged_holons.borrow().get_index_by_key(&key)?;
        self.to_validated_staged_reference(index)
    }

    fn staged_count(&self) -> i64 {
        self.staged_holons.borrow().len() as i64
    }

    fn stage_new_from_clone(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: HolonReference,
    ) -> Result<StagedReference, HolonError> {
        Err(HolonError::NotImplemented("stage_new_from_clone not implemented".to_string()))
    }

    fn stage_new_holon(
        &self,
        _context: &dyn HolonsContextBehavior,
        holon: Holon,
    ) -> Result<StagedReference, HolonError> {
        let new_index = self.stage_holon(holon);
        self.to_validated_staged_reference(new_index)
    }

    fn stage_new_version(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        Err(HolonError::NotImplemented("stage_new_version not implemented".to_string()))
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

    fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
        self.staged_holons.borrow().get_index_by_key(key)
    }

    /// Exports the staged holons using `SerializableHolonPool`
    fn export_staged_holons(&self) -> SerializableHolonPool {
        self.staged_holons.borrow().export_pool()
    }

    fn import_staged_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.staged_holons.borrow_mut().import_pool(pool); // Mutates existing HolonPool
    }

    /// Returns a reference to the staged Holons in the `HolonPool`,
    /// ensuring that commit functions can access the actual Holon instances.
    fn get_holons_to_commit(&self) -> Ref<Vec<Rc<RefCell<Holon>>>> {
        Ref::map(self.staged_holons.borrow(), |pool| pool.get_all_holons())
    }

    // fn stage_holon(&self, holon: Holon) -> usize {
    //     self.staged_holons.borrow_mut().insert_holon(holon)
    // }
}
