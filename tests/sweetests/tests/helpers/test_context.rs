use holons_client::ClientHolonService;

use holons_prelude::prelude::*;

use holons_core::{
    core_shared_objects::holon_pool::SerializableHolonPool,
    core_shared_objects::space_manager::HolonSpaceManager,
    core_shared_objects::transactions::{TransactionContext, TxId},
    core_shared_objects::transient_manager_access::TransientManagerAccess,
    {HolonCacheAccess, HolonServiceApi, NurseryAccess, ServiceRoutingPolicy, TransientCollection},
};

use holons_test::dance_test_language::DancesTestCase;

use std::sync::{Arc, RwLock};
use tracing::info;

use crate::helpers::init_tracing;

use super::create_test_dance_initiator;

/// The implementation of `HolonsContextBehavior` , responsible for managing
/// holon-related operations in a Sweetest environment.
///
/// The `TestHolonsContext` wraps a transaction-backed execution context for test flows.
#[derive(Debug)]
pub struct TestHolonsContext {
    /// The transaction-backed execution context used in tests.
    transaction_context: Arc<TransactionContext>,
}

/// Initializes a new fixture context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub fn init_fixture_context() -> Arc<dyn HolonsContextBehavior> {
    init_tracing(); // this sets tracing level for both fixture and test client

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

    // Wrap in `TestHolonsContext` and return as trait object.
    Arc::new(TestHolonsContext::new(transaction_context))
}

/// Initializes a new test context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub async fn init_test_context(test_case: &mut DancesTestCase) -> Arc<dyn HolonsContextBehavior> {
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

    // Step 5: Load transient holons from the test session state.
    transaction_context
        .import_transient_holons(test_case.test_session_state.get_transient_holons().clone());

    // Wrap in `TestHolonsContext` and return as trait object.
    Arc::new(TestHolonsContext::new(transaction_context))
}

impl TestHolonsContext {
    /// Creates a new `TestHolonsContext` from a provided `TransactionContext`.
    ///
    /// # Arguments
    /// * `transaction_context` - The transaction-backed execution context.
    ///
    /// # Returns
    /// * A new `TestHolonsContext` instance wrapping the provided transaction context.
    fn new(transaction_context: Arc<TransactionContext>) -> Self {
        Self { transaction_context }
    }
}

impl HolonsContextBehavior for TestHolonsContext {
    fn tx_id(&self) -> TxId {
        self.transaction_context.tx_id()
    }

    fn is_open(&self) -> bool {
        self.transaction_context.is_open()
    }

    fn get_nursery_access(&self) -> Arc<dyn NurseryAccess + Send + Sync> {
        self.transaction_context.get_nursery_access()
    }

    fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync> {
        self.transaction_context.get_staging_service()
    }

    fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.transaction_context.export_staged_holons()
    }

    fn import_staged_holons(&self, staged_holons: SerializableHolonPool) {
        self.transaction_context.import_staged_holons(staged_holons);
    }

    fn get_transient_behavior_service(&self) -> Arc<dyn TransientHolonBehavior + Send + Sync> {
        self.transaction_context.get_transient_behavior_service()
    }

    fn get_transient_manager_access(&self) -> Arc<dyn TransientManagerAccess + Send + Sync> {
        self.transaction_context.get_transient_manager_access()
    }

    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.transaction_context.export_transient_holons()
    }

    fn import_transient_holons(&self, transient_holons: SerializableHolonPool) {
        self.transaction_context.import_transient_holons(transient_holons);
    }

    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        self.transaction_context.get_cache_access()
    }

    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync> {
        self.transaction_context.get_holon_service()
    }

    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError> {
        self.transaction_context.get_dance_initiator()
    }

    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError> {
        self.transaction_context.get_space_holon()
    }

    fn set_space_holon(&self, space: HolonReference) -> Result<(), HolonError> {
        self.transaction_context.set_space_holon(space)
    }

    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>> {
        self.transaction_context.get_transient_state()
    }
}
