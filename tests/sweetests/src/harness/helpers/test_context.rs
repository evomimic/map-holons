use super::create_test_dance_initiator;
use crate::{build_core_schema_bootstrap_content_set, init_tracing, DancesTestCase};
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::reference_layer::HolonSpaceBehavior;
use holons_core::{HolonServiceApi, ServiceRoutingPolicy};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime, RuntimeSession};
use std::sync::Arc;
use std::time::Instant;
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
    let initialization_started = Instant::now();

    info!("\n ========== Initializing TEST RUNTIME ============");

    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Setup DanceInitiator
    let dance_initiator = create_test_dance_initiator().await;
    info!(
        elapsed_ms = initialization_started.elapsed().as_millis(),
        "sweettest runtime: dance initiator ready"
    );

    // Step 3: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        Some(dance_initiator),
        holon_service,
        None, // No local space holon initially
        ServiceRoutingPolicy::Combined,
    ));

    // Step 4: Create RuntimeSession and Runtime
    let session = Arc::new(RuntimeSession::new(Arc::clone(&space_manager), None));
    let runtime = Runtime::new(session);

    // Step 5: Bootstrap the first space through the same internal transaction
    // shape Conductora uses. The guest writes and returns CoreSchemaSpace as
    // the LocalHolonSpace anchor when this load commits.
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("failed to begin bootstrap transaction");
    let bootstrap_tx_id = match result {
        MapResult::TransactionCreated { tx_id } => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    };
    let bootstrap_context = runtime
        .session()
        .get_transaction(&bootstrap_tx_id)
        .expect("failed to get bootstrap transaction");
    bootstrap_context.enable_bootstrap_provisioning();
    let bootstrap_content_started = Instant::now();
    let bootstrap_content_set = build_core_schema_bootstrap_content_set()
        .expect("failed to build Core Schema bootstrap ContentSet");
    info!(
        elapsed_ms = bootstrap_content_started.elapsed().as_millis(),
        "sweettest runtime: CoreSchemaSpace bootstrap inputs ready"
    );
    let bootstrap_load_started = Instant::now();
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: bootstrap_context,
                action: TransactionAction::LoadHolons { content_set: bootstrap_content_set },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("failed to load Core Schema bootstrap bundle");
    info!(
        elapsed_ms = bootstrap_load_started.elapsed().as_millis(),
        "sweettest runtime: CoreSchemaSpace bootstrap load complete"
    );
    runtime
        .session()
        .archive_transaction(&bootstrap_tx_id)
        .expect("failed to archive bootstrap transaction");

    assert!(
        runtime
            .session()
            .space_manager()
            .get_space_holon_id()
            .expect("failed to read CoreSchemaSpace anchor")
            .is_some(),
        "Core Schema bootstrap did not inject a LocalHolonSpace anchor"
    );

    // Step 6: Begin the fixture's first ordinary transaction only after the
    // persisted CoreSchemaSpace anchor is available.
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("failed to begin initial transaction");
    let tx_id = match result {
        MapResult::TransactionCreated { tx_id } => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    };

    // Step 7: Import transient holons from fixture phase
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

    info!(
        elapsed_ms = initialization_started.elapsed().as_millis(),
        "sweettest runtime: initialization complete"
    );

    (runtime, tx_id)
}
