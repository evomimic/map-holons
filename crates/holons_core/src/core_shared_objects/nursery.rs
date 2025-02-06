use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::{Holon, HolonError, HolonState, NurseryAccess, RelationshipMap};
use crate::reference_layer::staged_reference::StagedIndex;
use crate::reference_layer::{
    HolonReference, HolonStagingBehavior, HolonsContextBehavior, SmartReference, StagedReference,
};
use hdi::prelude::*;
use shared_types_holon::{HolonId, MapString};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
// #[hdk_entry_helper]
// #[derive(Clone, PartialEq, Eq)]
// pub struct Nursery {
//     staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
//     keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
// }

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct Nursery {
    staged_holons: RefCell<StagedHolons>,
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct StagedHolons {
    staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

impl Nursery {
    /// Initializes a `Nursery` from a set of staged holons and a keyed index
    ///
    /// # Arguments
    ///
    /// * `staged_holons` - A vector of staged holons.
    /// * `keyed_index` - A map of keys to indices into the staged_holons vector
    ///
    /// # Returns
    ///
    /// A `Nursery` instance initialized with the provided holons and keyed index.
    /// Creates a new Nursery from a given stage
    pub fn new_from_staged_holons(
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
    ) -> Self {
        Self { staged_holons: RefCell::new(StagedHolons { staged_holons, keyed_index }) }
    }

    fn clone_existing_relationships_into_staged_map(
        &self,
        _original_holon: HolonId,
        _staged_holon: &Holon,
    ) -> Result<Rc<RelationshipMap>, HolonError> {
        todo!()
    }

    /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    fn to_validated_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Ok(staged_holon) = self.get_holon_by_index(staged_index) {
            let holon = &staged_holon.borrow();
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
        let staged_holons = self.staged_holons.borrow();
        index < staged_holons.staged_holons.len()
    }
}
impl NurseryAccess for Nursery {
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        let staged_holons = self.staged_holons.borrow();
        staged_holons.staged_holons.get(index).cloned().ok_or_else(|| {
            HolonError::IndexOutOfRange(format!(
                "Invalid index: {}. No staged holon at this position.",
                index
            ))
        })
    }

    /// Provides internal access to the nursery as `Rc<RefCell<dyn NurseryAccessInternal>>`.
    fn as_internal(&self) -> Rc<RefCell<dyn NurseryAccessInternal>> {
        Rc::new(RefCell::new(self.clone())) // Wrap `self` in `Rc<RefCell>`
    }
}

impl HolonStagingBehavior for Nursery {
    fn get_staged_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError> {
        let staged_holons = self.staged_holons.borrow();
        let index = staged_holons.get_index_by_key(&key)?;
        self.to_validated_staged_reference(index)
    }

    fn stage_new_from_clone(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: HolonReference,
    ) -> Result<StagedReference, HolonError> {
        Err(HolonError::NotImplemented(
            "the Nursery doesn't implement stage_new_from_clone yet".to_string(),
        ))
        // let cloned_holon = original_holon.clone_holon(context) {
        //     HolonReference::Staged(staged_reference) => {
        //         // Get a clone from the rc_holon in the commit_manager
        //         staged_reference.stage_new_from_clone(context)?
        //     }
        //     HolonReference::Smart(smart_reference) => {
        //         // Get a clone from the rc_holon in the cache_manager
        //         smart_reference.stage_new_from_clone(context)?
        //     }
        // };
        //
        // let cloned_staged_reference = {
        //     // Mutably borrow the commit_manager
        //     let space_manager = context.get_space_manager();
        //     // Stage the clone
        //     space_manager.stage_new_holon(cloned_holon)?
        // };
        //
        // // Reset the PREDECESSOR to None
        // cloned_staged_reference.with_predecessor(context, None)?;
        //
        // Ok(cloned_staged_reference)
    }

    fn stage_new_holon(
        &self,
        _context: &dyn HolonsContextBehavior,
        holon: Holon,
    ) -> Result<StagedReference, HolonError> {
        let new_index = self.stage_holon(&holon);
        self.to_validated_staged_reference(new_index)
    }

    fn stage_new_version(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        Err(HolonError::NotImplemented(
            "the Nursery doesn't implement stage_new_version yet".to_string(),
        ))
    }
}

impl NurseryAccessInternal for Nursery {
    fn clear_stage(&mut self) {
        let mut staged_holons = self.staged_holons.borrow_mut();
        staged_holons.keyed_index.clear();
        staged_holons.staged_holons.clear();
    }

    fn get_keyed_index(&self) -> BTreeMap<MapString, usize> {
        let staged_holons = self.staged_holons.borrow();
        staged_holons.keyed_index.clone()
    }

    fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
        let staged_holons = self.staged_holons.borrow();
        staged_holons.keyed_index.get(key).cloned().ok_or_else(|| {
            HolonError::HolonNotFound(format!("No staged holon found for key: {}", key))
        })
    }

    fn get_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        let staged_holons = self.staged_holons.borrow();
        staged_holons.staged_holons.clone()
    }

    fn stage_holon(&self, holon: &Holon) -> usize {
        // Create a new `Rc<RefCell<Holon>>` for the given `holon`
        let rc_holon = Rc::new(RefCell::new(holon.clone()));

        // Mutably borrow the RefCell
        let mut staged_holons = self.staged_holons.borrow_mut();

        // Add the new holon to the staged_holons and get its index
        let staged_index = staged_holons.staged_holons.len();
        staged_holons.staged_holons.push(rc_holon);

        // If the holon has a key, update the keyed index
        if let Ok(Some(key)) = holon.get_key() {
            staged_holons.keyed_index.insert(key, staged_index);
        }

        staged_index
    }
}
impl StagedHolons {
    // TODO: This implementation will need to be expanded to cover NurseryAccess once definition
    // Once Nursery has been upgraded to use StagedHolons

    /// Finds a holon by its key and returns its index.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// `Ok(usize)` containing the index if the key exists, or an `Err` if the key is not found.
    pub fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
        self.keyed_index.get(key).cloned().ok_or_else(|| {
            HolonError::HolonNotFound(format!("No staged holon found for key: {}", key))
        })
    }

    /// Retrieves a holon by its index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the holon to retrieve.
    ///
    /// # Returns
    ///
    /// `Ok(Rc<RefCell<Holon>>)` containing the holon if the index is valid, or an `Err` if the index is out of range.
    pub fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.staged_holons
            .get(index)
            .cloned()
            .ok_or_else(|| HolonError::IndexOutOfRange(format!("No holon at index {}", index)))
    }
}
