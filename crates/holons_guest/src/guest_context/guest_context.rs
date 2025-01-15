use crate::GuestHolonSpaceManager;
use holons_core::{
    Holon, HolonError, HolonReference, HolonSpaceBehavior, HolonsContextBehavior,
    TransientCollection, TransientCollectionBehavior,
};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// HolonsContext provides a single place to store information useful within a dance request.
pub struct GuestHolonsContext {
    dance_state: RefCell<TransientCollection>,
    space_manager: Rc<GuestHolonSpaceManager>,
}

impl GuestHolonsContext {
    /// Creates a new instance of `HolonsContext`.
    ///
    /// # Arguments
    /// * `space_manager` - The space manager to be associated with this context.
    pub fn new(space_manager: GuestHolonSpaceManager) -> Self {
        Self {
            dance_state: RefCell::new(TransientCollection::new()),
            space_manager: Rc::new(space_manager),
        }
    }

    /// Initializes a `HolonsContext` from session data.
    ///
    /// # Arguments
    /// * `staged_holons` - A vector of staged holons wrapped in `Rc<RefCell>`.
    /// * `keyed_index` - A map of keys to their corresponding indices in the staged holons.
    /// * `local_space_holon` - An optional reference to the local space holon.
    ///
    /// # Returns
    /// A `Result` containing the new `HolonsContext` or an error.
    pub fn init_context_from_session(
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        local_space_holon: Option<HolonReference>,
    ) -> Result<Self, HolonError> {
        // Step 1: Initialize the HolonSpaceManager
        let mut space_manager =
            GuestHolonSpaceManager::new_from_session(staged_holons, keyed_index, local_space_holon);

        // Step 2: Ensure the local holon space is available
        let context = GuestHolonsContext::new(space_manager.clone());
        let _local_space_holon = space_manager.ensure_local_holon_space(&context)?;

        // Step 3: Return the initialized HolonsContext
        Ok(context)
    }
}

impl HolonsContextBehavior for GuestHolonsContext {
    /// Attempts to retrieve a clone of the Local Space Holon reference from the space manager.
    fn get_local_space_holon(&self) -> Option<HolonReference> {
        self.get_space_manager().get_space_holon()
    }

    /// Provides access to the space manager as a trait object.
    fn get_space_manager(&self) -> Rc<&dyn HolonSpaceBehavior> {
        // Create a reference to the HolonSpaceManager as a trait object
        let reference: &dyn HolonSpaceBehavior = &*self.space_manager;
        // Wrap the reference in Rc
        Rc::new(reference)
    }

    fn add_references_to_dance_state(&self, holons: Vec<HolonReference>) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_references(self, holons)
    }

    fn add_reference_to_dance_state(&self, holon_ref: HolonReference) -> Result<(), HolonError> {
        self.dance_state.borrow_mut().add_reference(self, holon_ref)
    }

    fn get_by_key_from_dance_state(
        &self,
        key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.dance_state.borrow().get_by_key(key)
    }
}
