use crate::client_shared_objects::ClientHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::ServiceRoutingPolicy;
use holons_core::core_shared_objects::transactions::TransactionContext;

use holons_core::dances::DanceInitiator;
use holons_core::reference_layer::HolonServiceApi;

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
/// * An `Arc<TransactionContext>` backed by a `TransactionContext`.
pub fn init_client_context(
    initiator: Option<Arc<dyn DanceInitiator>>,
) -> Arc<TransactionContext> {
    // Create the ClientHolonService.
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Create a new `HolonSpaceManager` wrapped in `Arc`.
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        initiator,     // Dance initiator for conductor calls
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
    ));

    // Open the default transaction for this space.
    // TransactionContext becomes the sole execution root and owns the space.
    let transaction_context = space_manager
        .get_transaction_manager()
        .open_default_transaction(Arc::clone(&space_manager))
        .expect("failed to open default client transaction");

    transaction_context
}
