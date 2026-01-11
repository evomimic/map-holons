use crate::client_shared_objects::ClientHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{Nursery, ServiceRoutingPolicy, TransientHolonManager};

use holons_core::dances::DanceInitiator;
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};

use std::sync::Arc;

/// Initializes a new client-side context with a fresh `HolonSpaceManager` and
/// an implicit default transaction.
///
/// This function sets up:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with client-specific routing policies.
/// - An implicit transaction opened via the per-space `TransactionManager`.
/// - Injects the optional `DanceInitiator` for conductor calls.
///
/// # Returns
/// * An `Arc<dyn HolonsContextBehavior>` backed by a `TransactionContext`.
pub fn init_client_context(
    initiator: Option<Arc<dyn DanceInitiator>>,
) -> Arc<dyn HolonsContextBehavior + Send + Sync> {
    // Step 1: Create the ClientHolonService.
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Create empty staging/transient pools required by the legacy space manager.
    let nursery = Nursery::new();

    // Step 3: Create an empty TransientHolonManager for the client.
    let transient_manager = TransientHolonManager::new_empty();

    // Step 4: Setup Conductor and Construct the DanceInitiator.
    // let conductor_config = setup_conductor().await; // Temporarily using mock conductor
    // let dance_initiator: Arc<dyn DanceInitiatorApi> =
    //     Arc::new(DanceInitiator::new(Arc::new(conductor_config)));
    // let client_dance_caller = ClientDanceCaller::new(Arc::new(conductor));
    // let dance_initiator: Arc<dyn DanceInitiatorApi> =
    //     Arc::new(DanceInitiator::new(Arc::new(client_dance_caller)));

    // Step 5: Create a new `HolonSpaceManager` wrapped in `Arc`.
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        initiator,     // Dance initiator for conductor calls
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
        nursery,
        transient_manager,
    ));

    // Step 6: Open the default transaction for this space.
    let transaction_context = space_manager
        .get_transaction_manager()
        .open_default_transaction(Arc::clone(&space_manager))
        .expect("failed to open default client transaction");

    transaction_context
}
