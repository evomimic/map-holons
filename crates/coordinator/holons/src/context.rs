//use crate::cache_manager::HolonCacheManager;
//use crate::commit_manager::CommitManager;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::space_manager::SpaceManager;
use crate::transient_collection::TransientCollection;
use shared_types_holon::MapString;
use std::cell::RefCell;
/// HolonsContext provides a single place to information useful within a dance request
pub struct HolonsContext {
   // pub commit_manager: RefCell<CommitManager>,
   // pub cache_manager: RefCell<HolonCacheManager>,
    pub dance_state: RefCell<TransientCollection>,
    pub local_space_manager: RefCell<SpaceManager>,
    //pub ext_space_managers: HashMap<HolonSpaceId,RefCell<SpaceManager>>,
    pub local_space_holon: RefCell<Option<HolonReference>>
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            //commit_manager: CommitManager::new().into(),
            //cache_manager: HolonCacheManager::new().into(),
            dance_state: TransientCollection::new().into(),
            local_space_manager: SpaceManager::new().into(),
            local_space_holon: RefCell::new(None)
        }
    }
    pub fn init_context(
        //commit_manager: CommitManager,
        //cache_manager: HolonCacheManager,
        local_space_manager: SpaceManager,
        local_space_holon: Option<HolonReference>,
    ) -> HolonsContext {
        // Return the initialized context
        HolonsContext {
          //  commit_manager: RefCell::from(commit_manager),
          //  cache_manager: RefCell::from(cache_manager),
            dance_state: TransientCollection::new().into(),
            local_space_manager: RefCell::from(local_space_manager),
            local_space_holon: RefCell::new(local_space_holon),
        }
    }

    /// This method returns a clone of the Local Space Holon reference from the context
    /// NOTE: This will panic on borrow failure
    pub fn get_local_space_holon(&self) -> Option<HolonReference> {
        let local_space_holon = self.local_space_holon.borrow();
        local_space_holon.clone() // If no panic, return cloned value
    }

    /// This method sets the Local Space holon reference within the context
    pub fn set_local_space_holon(&self, new_holon_space: HolonReference) -> Result<(), HolonError> {
        match self.local_space_holon.try_borrow_mut() {
            Ok(mut local_space_holon) => {
                *local_space_holon = Some(new_holon_space); // Successfully borrowed and mutated
                Ok(())
            }
            Err(_) => {
                Err(HolonError::FailedToBorrow("Failed to borrow local_holon_space mutably".into()))
            }
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
