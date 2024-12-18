use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::query_manager::QueryManager;
use crate::space_manager::HolonSpaceManager;
use crate::transient_collection::TransientCollection;
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::rc::Rc;
/// HolonsContext provides a single place to information useful within a dance request
pub struct HolonsContext {
    pub dance_state: RefCell<TransientCollection>,
    pub space_manager: Rc<RefCell<HolonSpaceManager>>,
    pub query_manager: RefCell<QueryManager>
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        let space_manager = Rc::from(RefCell::from(HolonSpaceManager::new()));
        let query_manager = QueryManager::new(Rc::clone(&space_manager)).into();
        HolonsContext {
            dance_state: TransientCollection::new().into(),
            space_manager, //Rc::from(HolonSpaceManager::new().into()),
            query_manager,// QueryManager::new(Rc.clone(space_manager)).into()
        }
    }
    pub fn init_context(
        space_manager: HolonSpaceManager,
    ) -> HolonsContext {
        // Return the initialized context
        let space_manager = Rc::from(RefCell::from(space_manager));
        let query_manager = RefCell::from(QueryManager::new(Rc::clone(&space_manager)));
        HolonsContext {
            dance_state: TransientCollection::new().into(),
            space_manager,//: Rc::from(RefCell::from(space_manager)),
            query_manager //QueryManager::new(Rc::new(RefCell::new(space_manager))).into()
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
