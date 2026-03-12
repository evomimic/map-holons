use crate::client_shared_objects::ClientHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::ServiceRoutingPolicy;

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
pub fn init_client_context(initiator: Option<Arc<dyn DanceInitiator>>) -> Arc<TransactionContext> {
    let space_manager = init_client_runtime(initiator);

    // Open the default transaction for this space.
    // TransactionContext becomes the sole execution root and owns the space.
    space_manager
        .get_transaction_manager()
        .open_new_transaction(Arc::clone(&space_manager))
        .expect("failed to open default client transaction")
}

/// Initializes a new client-side `HolonSpaceManager` without opening a transaction.
///
/// Same construction as `init_client_context()` but returns the space manager
/// directly, leaving transaction lifecycle to the caller (e.g., `RuntimeSession`).
pub fn init_client_runtime(initiator: Option<Arc<dyn DanceInitiator>>) -> Arc<HolonSpaceManager> {
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    Arc::new(HolonSpaceManager::new_with_managers(
        initiator,
        holon_service,
        None,
        ServiceRoutingPolicy::Combined,
    ))
}
