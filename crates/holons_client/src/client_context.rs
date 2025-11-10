use crate::client_shared_objects::ClientHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{Nursery, ServiceRoutingPolicy, TransientHolonManager};

use holons_core::reference_layer::{HolonServiceApi, HolonSpaceBehavior, HolonsContextBehavior};
use std::fmt::Debug;
use std::sync::Arc;
use tracing::warn;

/// The client-side implementation of `HolonsContextBehavior`, responsible for managing
/// holon-related operations in a local (non-guest) environment.
///
/// The `ClientHolonsContext` owns an instance of `HolonSpaceManager`, wrapped in `Arc`
/// for shared ownership, ensuring that it can be referenced safely across different
/// parts of the application without requiring mutable access.
///
/// This context is **only used on the client-side** and does not interact with Holochain directly.
#[derive(Debug)]
pub struct ClientHolonsContext {
    /// The `HolonSpaceManager` that provides access to all core services.
    space_manager: Arc<HolonSpaceManager>,
}

/// Initializes a new client-side context with a fresh `HolonSpaceManager`.
///
/// This function sets up a `ClientHolonsContext` with:
/// - An **empty nursery** (no staged holons).
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with client-specific routing policies.
/// - Shared ownership support via `Arc<dyn HolonsContextBehavior>`, allowing multiple components
///   to reference the same context without unnecessary cloning.
/// - Injects the **DanceInitiator**, backed by a client-side implementation `ConductorDanceCaller`
///
/// # Returns
/// * An `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub async fn init_client_context() -> Arc<dyn HolonsContextBehavior> {
    warn!("\n ========== Initializing CLIENT CONTEXT ============");
    // Step 1: Create the ClientHolonService
    // temporarily create with injected DanceCallService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Create an empty Nursery for the client
    let nursery = Nursery::new();

    // Step 3: Create an empty TransientHolonManager for the client
    let transient_manager = TransientHolonManager::new_empty();

    // Step 4: Setup Conductor and Construct the DanceInitiator
    // let conductor_config = setup_conductor().await; // Temporarily using mock conductor
    // let dance_initiator: Arc<dyn DanceInitiatorApi> =
    //     Arc::new(DanceInitiator::new(Arc::new(conductor_config)));
    // let client_dance_caller = ClientDanceCaller::new(Arc::new(conductor));
    // let dance_initiator: Arc<dyn DanceInitiatorApi> =
    //     Arc::new(DanceInitiator::new(Arc::new(client_dance_caller)));

    // Step 4: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
        nursery,
        transient_manager,
    ));

    // Wrap in `ClientHolonsContext` and return as an Arc<dyn HolonsContextBehavior>
    Arc::new(ClientHolonsContext::new(space_manager))
}
impl ClientHolonsContext {
    /// Creates a new `ClientHolonsContext` from a provided `HolonSpaceManager`.
    ///
    /// # Arguments
    /// * `space_manager` - The `HolonSpaceManager` instance to be associated with this context.
    ///
    /// # Returns
    /// * A new `ClientHolonsContext` instance wrapping the provided space manager.
    pub fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager }
    }
}

impl HolonsContextBehavior for ClientHolonsContext {
    /// Provides access to the `HolonSpaceManager` as a shared reference.
    ///
    /// # Returns
    /// * `Arc<dyn HolonSpaceBehavior>` - A shared reference to the space manager.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior> {
        self.space_manager.clone() as Arc<dyn HolonSpaceBehavior>
    }
}
