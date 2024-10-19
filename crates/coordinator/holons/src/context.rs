use crate::cache_manager::HolonCacheManager;
use crate::commit_manager::CommitManager;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::transient_collection::TransientCollection;
use shared_types_holon::MapString;
use std::cell::RefCell;
/// HolonsContext provides a single place to information useful within a dance request
pub struct HolonsContext {
    pub commit_manager: RefCell<CommitManager>,
    pub cache_manager: RefCell<HolonCacheManager>,
    pub dance_state: RefCell<TransientCollection>,
    pub local_holon_space: RefCell<Option<HolonReference>>,
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            commit_manager: CommitManager::new().into(),
            cache_manager: HolonCacheManager::new().into(),
            dance_state: TransientCollection::new().into(),
            local_holon_space: RefCell::new(None),
        }
    }
    pub fn init_context(
        commit_manager: CommitManager,
        cache_manager: HolonCacheManager,
        local_holon_space: Option<HolonReference>,
    ) -> HolonsContext {
        // Return the initialized context
        HolonsContext {
            commit_manager: RefCell::from(commit_manager),
            cache_manager: RefCell::from(cache_manager),
            dance_state: TransientCollection::new().into(),
            local_holon_space: RefCell::new(local_holon_space),
        }
    }
    /// This method returns a clone of the LocalHolonSpace reference from the context
    /// NOTE: This will panic on borrow failure
    pub fn get_local_holon_space(&self) -> Option<HolonReference> {
        let local_holon_space = self.local_holon_space.borrow();
        local_holon_space.clone() // If no panic, return cloned value
    }

    /// This method sets the LocalHolonSpace reference within the context
    pub fn set_local_holon_space(&self, new_holon_space: HolonReference) -> Result<(), HolonError> {
        match self.local_holon_space.try_borrow_mut() {
            Ok(mut local_holon_space) => {
                *local_holon_space = Some(new_holon_space); // Successfully borrowed and mutated
                Ok(())
            }
            Err(_) => Err(HolonError::FailedToBorrow(
                "Failed to borrow local_holon_space mutably".into(),
            )),
        }
    }
    pub fn add_references_to_dance_state(
        &self,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_references(self, holons)
    }

    pub fn add_reference_to_dance_state(
        &self,
        holon_ref: HolonReference,
    ) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_reference(self, holon_ref)
    }

    pub fn get_by_key_from_dance_state(
        &self,
        key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.dance_state.borrow().get_by_key(key)
    }
}
