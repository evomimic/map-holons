use holochain::prelude::DbKind::Test;
use holons_client::client_context::ClientHolonsContext;
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{HolonError, Nursery, ServiceRoutingPolicy};
use holons_core::reference_layer::{HolonServiceApi, HolonSpaceBehavior, HolonsContextBehavior};
use std::cell::RefCell;
use std::sync::Arc;

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

/// The TestHolonsContext can be configured for `TestFixture` usage (running in the Holochain Test
/// Orchestrator) or for `TestExecution` usage (running in the Holochain Mock Conductor)
pub enum TestContextConfigOption {
    TestFixture,
    TestExecution,
}

/// Initializes a new test context with a fresh `HolonSpaceManager`.
///
/// Under the TestFixture configuration option, this function sets up a `TestHolonsContext` with:
/// - An **empty nursery** (no staged holons).
/// - A default `HolonServiceApi` implementation (`ClientHolonService`).
/// - A space manager configured with guest-specific routing policies.
///
/// # Returns
/// * A `Arc<dyn HolonsContextBehavior>` containing the initialized client context.
pub fn init_test_context(
    _config_option: TestContextConfigOption,
) -> Arc<dyn HolonsContextBehavior> {
    // Step 1: Create the ClientHolonService
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    // Step 2: Create an empty Nursery for the client
    let nursery = Nursery::new();

    // Create a new `HolonSpaceManager` wrapped in `Arc`
    let space_manager = Arc::new(HolonSpaceManager::new_with_nursery(
        holon_service, // Service for holons
        None,          // No local space holon initially
        ServiceRoutingPolicy::Combined,
        nursery,
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
