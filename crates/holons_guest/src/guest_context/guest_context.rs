use crate::guest_shared_objects::GuestHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{
    Holon, HolonError, ServiceRoutingPolicy, TransientCollection,
};
use holons_core::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonsContextBehavior,
    TransientCollectionBehavior,
};
use shared_types_holon::{HolonId, MapString};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// The `GuestHolonsContext` provides a guest-specific implementation of the `HolonsContextBehavior`
/// trait, offering functionality necessary for Holochain guest-side operations.
/// It includes both a `HolonSpaceManager` and a `TransientCollection` to manage the state
/// and lifecycle of holons during a request.
pub struct GuestHolonsContext {
    dance_state: RefCell<TransientCollection>,
    space_manager: Rc<HolonSpaceManager>,
}

impl GuestHolonsContext {
    /// Creates a new instance of `GuestHolonsContext` with the given `HolonSpaceManager`.
    ///
    /// # Arguments
    /// * `space_manager` - The `HolonSpaceManager` responsible for managing holons.
    pub fn new(space_manager: Rc<HolonSpaceManager>) -> Self {
        Self { dance_state: RefCell::new(TransientCollection::new()), space_manager }
    }

    /// Initializes a `HolonsContext` from session data, injecting a `GuestHolonService` object.
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
        // Step 1: Construct the concrete GuestHolonService instance
        let guest_holon_service = GuestHolonService;

        // Step 2: Check if local_space_holon is None
        let mut local_space_holon = local_space_holon;

        if local_space_holon.is_none() {
            // Directly invoke `ensure_local_holon_space` on the GuestHolonService instance
            let local_holon = guest_holon_service.ensure_local_holon_space()?;

            // Convert the Holon into a HolonReference
            let holon_id = HolonId::Local(local_holon.get_local_id()?);

            local_space_holon = Some(HolonReference::from_id(holon_id));
        }

        // Step 3: Wrap the GuestHolonService in an Arc<dyn HolonServiceApi>
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(guest_holon_service);

        // Step 4: Initialize the HolonSpaceManager
        let space_manager = Rc::new(HolonSpaceManager::new_from_session(
            holon_service,
            staged_holons,
            keyed_index,
            local_space_holon,
            ServiceRoutingPolicy::BlockExternal,
        ));

        // Step 5: Create the GuestHolonsContext
        let context = GuestHolonsContext::new(space_manager);

        // Step 6: Return the initialized HolonsContext
        Ok(context)
    }
}

impl HolonsContextBehavior for GuestHolonsContext {
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
