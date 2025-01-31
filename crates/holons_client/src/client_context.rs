// use holochain::prelude::*;

use crate::client_shared_objects::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{
    Holon, HolonError, ServiceRoutingPolicy, TransientCollection,
};
use holons_core::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonsContextBehavior,
    TransientCollectionBehavior,
};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// HolonsContext provides a single place to store information useful within a dance request.
pub struct ClientHolonsContext {
    space_manager: Box<HolonSpaceManager>,
}

impl ClientHolonsContext {
    /// Creates a new instance of `HolonsContext`.
    ///
    /// # Arguments
    /// * `space_manager` - The space manager to be associated with this context.
    pub fn new(space_manager: HolonSpaceManager) -> Self {
        Self { space_manager: Box::new(space_manager) }
    }

    /// Initializes a `HolonsContext`
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
        // Use Arc instead of Box to wrap GuestHolonService
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);
        // Step 1: Initialize the HolonSpaceManager
        let space_manager = HolonSpaceManager::new_from_session(
            holon_service,
            staged_holons,
            keyed_index,
            local_space_holon,
            ServiceRoutingPolicy::Combined,
        );

        // Step 2: Ensure the local holon space is available
        let context = ClientHolonsContext::new(space_manager);

        // Step 3: Return the initialized HolonsContext
        Ok(context)
    }
}

impl HolonsContextBehavior for ClientHolonsContext {
    /// Provides access to the space manager as a trait object.
    fn get_space_manager(&self) -> Rc<&dyn HolonSpaceBehavior> {
        // Create a reference to the HolonSpaceManager as a trait object
        let reference: &dyn HolonSpaceBehavior = &*self.space_manager;
        // Wrap the reference in Rc
        Rc::new(reference)
    }
}
