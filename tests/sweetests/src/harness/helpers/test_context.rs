use super::create_test_dance_initiator;
use crate::init_tracing;
use crate::DancesTestCase;
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{HolonServiceApi, ServiceRoutingPolicy};
use std::sync::Arc;
use tracing::info;

/// Initializes a new fixture context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<TransactionContext>` containing the initialized client context.
pub fn init_fixture_context() -> Arc<TransactionContext> {
    // this sets tracing level for both fixture and test client
    // this method is idempotent by design
    init_tracing();

    info!("\n ========== Initializing FIXTURE CONTEXT ============");

    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Setup trust channel and Inject DanceInitiator
    // SKIP -- Fixtures cannot initiate dances!

    // Step 3: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
    ));

    // Step 4: Open the default transaction for this space.
    let transaction_context = space_manager
        .get_transaction_manager()
        .open_default_transaction(Arc::clone(&space_manager))
        .expect("failed to open default fixture transaction");

    transaction_context
}

/// Initializes a new test context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<TransactionContext>` containing the initialized client context.
pub async fn init_test_context(test_case: &mut DancesTestCase) -> Arc<TransactionContext> {
    init_tracing();

    info!("\n ========== Initializing TEST CONTEXT ============");

    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Setup DanceInitiator
    let dance_initiator = create_test_dance_initiator().await;

    // Step 3: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        Some(dance_initiator),
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
    ));

    // Step 4: Open the default transaction for this space.
    let transaction_context = space_manager
        .get_transaction_manager()
        .open_default_transaction(Arc::clone(&space_manager))
        .expect("failed to open default test transaction");

    // Step 5: Load transient holons from the test session_state state.
    let bound_transient_holons = test_case
        .test_session_state
        .get_transient_holons()
        .clone()
        .bind(&transaction_context)
        .expect("failed to bind transient holon wire pool into runtime holon pool");

    transaction_context
        .import_transient_holons(bound_transient_holons)
        .expect("failed to import transient holons into test context");

    transaction_context
}
