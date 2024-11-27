use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::space_manager::HolonSpaceManager;
use crate::transient_collection::TransientCollection;
use shared_types_holon::MapString;
use std::cell::RefCell;
/// HolonsContext provides a single place to information useful within a dance request
pub struct HolonsContext {
    pub dance_state: RefCell<TransientCollection>,
    pub space_manager: RefCell<HolonSpaceManager>
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            dance_state: TransientCollection::new().into(),
            space_manager: HolonSpaceManager::new().into(),
        }
    }
    pub fn init_context(
        space_manager: HolonSpaceManager,
    ) -> HolonsContext {
        // Return the initialized context
        HolonsContext {
            dance_state: TransientCollection::new().into(),
            space_manager: RefCell::from(space_manager),
        }
    }

    /// This method returns a clone of the Local Space Holon reference from the space manager
    /// NOTE: This will panic on borrow failure
    pub fn get_local_space_holon(&self) -> Option<HolonReference> {
        let space_manager = self.space_manager.borrow();
        space_manager.get_space_holon()
       // local_space_holon.clone() // If no panic, return cloned value
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
