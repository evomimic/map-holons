use crate::cache_manager::HolonCacheManager;
use crate::commit_manager::CommitManager;
use std::cell::RefCell;
use shared_types_holon::MapString;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::transient_collection::TransientCollection;

pub struct HolonsContext {
    pub commit_manager: RefCell<CommitManager>,
    pub cache_manager: RefCell<HolonCacheManager>,
    pub dance_state: RefCell<TransientCollection>,
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            commit_manager: CommitManager::new().into(),
            cache_manager: HolonCacheManager::new().into(),
            dance_state: TransientCollection::new().into(),
        }
    }
    pub fn init_context(commit_manager: CommitManager, cache_manager: HolonCacheManager) -> HolonsContext {
        HolonsContext {
            commit_manager: RefCell::from(commit_manager),
            cache_manager: RefCell::from(cache_manager),
            dance_state: TransientCollection::new().into(),
        }
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
