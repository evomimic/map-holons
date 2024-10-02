use crate::cache_manager::HolonCacheManager;
use crate::commit_manager::CommitManager;
use crate::staged_reference::StagedReference;
use std::cell::{Ref, RefCell};
use shared_types_holon::MapString;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::transient_collection::TransientCollection;
/// HolonsContext provides a single place to information useful within a dance request
pub struct HolonsContext {
    pub commit_manager: RefCell<CommitManager>,
    pub cache_manager: RefCell<HolonCacheManager>,
    pub dance_state: RefCell<TransientCollection>,
    pub local_holon_space: Option<HolonReference>,
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            commit_manager: CommitManager::new().into(),
            cache_manager: HolonCacheManager::new().into(),
            dance_state: TransientCollection::new().into(),
            local_holon_space: None,
        }
    }
    pub fn init_context(
        commit_manager: CommitManager,
        cache_manager: HolonCacheManager,
        local_holon_space: Option<HolonReference>
    ) -> HolonsContext {

        // Return the initialized context
        HolonsContext {
            commit_manager: RefCell::from(commit_manager),
            cache_manager: RefCell::from(cache_manager),
            dance_state: TransientCollection::new().into(),
            local_holon_space: local_holon_space.clone(),
        }
    }
    /// This method returns a reference to the LocalHolonSpace
    pub fn get_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        self.local_holon_space.borrow()
    }
    fn set_local_holon_space(&self, new_holon_space: HolonReference) {
        // Borrow mutably and replace the None with Some(new_holon_space)
        *self.local_holon_space.borrow_mut() = Some(new_holon_space);
    }
    pub fn add_references_to_dance_state(&self, holons: Vec<HolonReference>) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_references(self, holons)
    }

    pub fn add_reference_to_dance_state(&self, holon_ref: HolonReference) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_reference(self, holon_ref)
    }

    pub fn get_by_key_from_dance_state(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError>  {
        self.dance_state.borrow().get_by_key(key)
    }

}
