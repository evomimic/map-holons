use super::create_test_dance_initiator;
use crate::{init_tracing, DancesTestCase};
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::{HolonServiceApi, ServiceRoutingPolicy};
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use map_commands_runtime::{Runtime, RuntimeSession};
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
        .open_new_transaction(Arc::clone(&space_manager))
        .expect("failed to open default fixture transaction");

    transaction_context
}

/// Initializes a `Runtime` with a `RuntimeSession` and begins the first transaction
/// through `Runtime::execute_command(SpaceCommand::BeginTransaction)`.
///
/// Fixture-created transient holons are imported into the initial transaction context.
///
/// # Returns
/// * `(Runtime, TxId)` — the runtime and the first transaction's id.
pub async fn init_test_runtime(test_case: &mut DancesTestCase) -> (Runtime, TxId) {
    init_tracing();

    info!("\n ========== Initializing TEST RUNTIME ============");

    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Setup DanceInitiator
    let dance_initiator = create_test_dance_initiator().await;

    // Step 3: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        Some(dance_initiator),
        holon_service,
        None, // No local space holon initially
        ServiceRoutingPolicy::Combined,
    ));

    // Step 4: Create RuntimeSession and Runtime
    let session = Arc::new(RuntimeSession::new(Arc::clone(&space_manager)));
    let runtime = Runtime::new(session);

    // Step 5: Begin first transaction through the real command path
    let result = runtime
        .execute_command(MapCommand::Space(SpaceCommand::BeginTransaction))
        .await
        .expect("failed to begin initial transaction");
    let tx_id = match result {
        MapResult::TransactionCreated { tx_id } => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    };

    // Step 6: Import transient holons from fixture phase
    let context =
        runtime.session().get_transaction(&tx_id).expect("failed to get initial transaction");
    let bound_transient_holons = test_case
        .test_session_state
        .get_transient_holons()
        .clone()
        .bind(&context)
        .expect("failed to bind transient holon wire pool into runtime holon pool");
    context
        .import_transient_holons(bound_transient_holons)
        .expect("failed to import transient holons into test context");

    (runtime, tx_id)
}
