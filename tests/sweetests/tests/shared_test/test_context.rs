use crate::shared_test::tracing_utils::init_tracing;
use crate::test_data_types::DancesTestCase;

use holochain::prelude::DbKind::Test;
use holons_client::client_context::ClientHolonsContext;
use holons_client::ClientHolonService;

use holons_core::dances::{DanceCallService, DanceCallServiceApi};
use holons_prelude::prelude::*;

use holons_core::core_shared_objects::holon_pool::TransientHolonPool;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::TransientHolonManager;
use holons_core::{setup_conductor, HolonPool, HolonServiceApi, Nursery, ServiceRoutingPolicy};
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{info, warn};

/// The implementation of `HolonsContextBehavior` , responsible for managing
/// holon-related operations in a Sweetest environment.
///
/// The `TestHolonsContext` owns an instance of `HolonSpaceManager`, wrapped in `Arc`
/// for shared ownership, ensuring that it can be referenced safely across different
/// parts of the application without requiring mutable access.
#[derive(Debug)]
pub struct TestHolonsContext {
    /// The `HolonSpaceManager` that provides access to all core services.
    space_manager: Arc<HolonSpaceManager>,
}

/// Initializes a new fixture context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - An **empty nursery** (no staged holons).
/// - An **empty transient_manager**.
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub async fn init_fixture_context() -> Arc<dyn HolonsContextBehavior> {
    init_tracing();
    warn!("\n ========== Tracing has been initialized ============");

    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Create an empty Nursery for the client
    let nursery = Nursery::new();

    // Step 3: Create an empty TransientHolonManager for the client
    let transient_manager = TransientHolonManager::new_empty();

    // Step 4: Setup SweetConductor (Mock) and Inject DanceCallService
    let conductor_config = setup_conductor().await;
    let dance_call_service: Arc<dyn DanceCallServiceApi> =
        Arc::new(DanceCallService::new(Arc::new(conductor_config)));

    // Step 5: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        dance_call_service,
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
        nursery,
        transient_manager,
    ));

    // Wrap in `TestHolonsContext` and return as trait object
    Arc::new(TestHolonsContext::new(space_manager))
}

/// Initializes a new test context with a fresh `HolonSpaceManager` with parameters:
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - An **empty nursery** (no staged holons).
/// - A populated transient_manager from the test_session_state.
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub async fn init_test_context(test_case: &mut DancesTestCase) -> Arc<dyn HolonsContextBehavior> {
    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Create an empty Nursery for the client
    let nursery = Nursery::new();

    // Step 3: Set transient holons in client TransientManager
    let transient_manager = TransientHolonManager::new_with_pool(TransientHolonPool(
        HolonPool::from(test_case.test_session_state.get_transient_holons().clone()),
    ));

    // Step 4: Setup SweetConductor (Mock) and Inject DanceCallService
    let conductor_config = setup_conductor().await;
    let dance_call_service: Arc<dyn DanceCallServiceApi> =
        Arc::new(DanceCallService::new(Arc::new(conductor_config)));

    // Step 5: Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        dance_call_service,
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
        nursery,
        transient_manager,
    ));

    // Wrap in `TestHolonsContext` and return as trait object
    Arc::new(TestHolonsContext::new(space_manager))
}

impl TestHolonsContext {
    /// Creates a new `TestHolonsContext` from a provided `HolonSpaceManager`.
    ///
    /// # Arguments
    /// * `space_manager` - The `HolonSpaceManager` instance to be associated with this context.
    ///
    /// # Returns
    /// * A new `TestHolonsContext` instance wrapping the provided space manager.
    fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager }
    }
}

impl HolonsContextBehavior for TestHolonsContext {
    /// Provides access to the `HolonSpaceManager` as a shared reference.
    ///
    /// # Returns
    /// * `Arc<dyn HolonSpaceBehavior>` - A shared reference to the space manager.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior> {
        self.space_manager.clone() as Arc<dyn HolonSpaceBehavior>
    }
}
