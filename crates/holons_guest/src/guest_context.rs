use crate::guest_shared_objects::GuestHolonService;
use std::cell::RefCell;

use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
use holons_core::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{HolonError, Nursery, ServiceRoutingPolicy};
use holons_core::reference_layer::{HolonReference, HolonSpaceBehavior, HolonsContextBehavior};
use std::sync::Arc;

/// The guest-side implementation of `HolonsContextBehavior`, responsible for managing
/// holon-related operations **within the Holochain guest environment**.
///
/// The `GuestHolonsContext` owns an instance of `HolonSpaceManager`, wrapped in `Arc`
/// for shared ownership. This ensures that the space manager can be accessed safely
/// across different parts of the guest runtime.
///
/// This context is **only used on the guest-side** and interacts directly with Holochain.
pub struct GuestHolonsContext {
    /// The `HolonSpaceManager` that provides access to all core services.
    space_manager: Arc<HolonSpaceManager>,
}

impl GuestHolonsContext {
    /// Creates a new `GuestHolonsContext` from a provided `HolonSpaceManager`.
    ///
    /// # Arguments
    /// * `space_manager` - The `HolonSpaceManager` instance to be associated with this context.
    ///
    /// # Returns
    /// * A new `GuestHolonsContext` instance wrapping the provided space manager.
    pub fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager }
    }
}

impl HolonsContextBehavior for GuestHolonsContext {
    /// Provides access to the `HolonSpaceManager` as a shared reference.
    ///
    /// # Returns
    /// * `Arc<dyn HolonSpaceBehavior>` - A shared reference to the space manager.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior> {
        self.space_manager.clone() as Arc<dyn HolonSpaceBehavior>
    }
}
/// Initializes a new guest-side context with a `HolonSpaceManager` configured for Holochain execution.
///
/// This function sets up a `GuestHolonsContext` with:
/// - **Staged holons** from the session state (if any).
/// - A `GuestHolonService` as the persistence and retrieval layer.
/// - A space manager configured with **guest-specific routing policies**.
/// - Internal nursery access, required for commit operations.
///
/// # Arguments
/// * `staged_holons` - The `SerializableHolonPool` containing staged holons from the session state.
/// * `local_space_holon` - An optional reference to the local holon space.
///
/// # Returns
/// * `Ok(Box<dyn HolonsContextBehavior>)` - The initialized guest context if successful.
/// * `Err(HolonError)` - If internal access registration fails.
///
/// # Errors
/// This function returns an error if it fails to acquire a mutable reference to `GuestHolonService`,
/// which indicates that multiple references exist to the same service instance, preventing modification.
pub fn init_guest_context(
    staged_holons: SerializableHolonPool,
    local_space_holon: Option<HolonReference>,
) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
    // Step 1: Create the GuestHolonService
    let mut guest_holon_service = Arc::new(GuestHolonService::new()); // ✅ Freshly created

    // Step 2: Create and initialize the Nursery
    let mut nursery = Nursery::new();
    nursery.import_staged_holons(staged_holons); // ✅ Load staged holons

    // Step 3: Register internal access
    let service = Arc::get_mut(&mut guest_holon_service).ok_or_else(|| {
        HolonError::FailedToBorrow(
            "Failed to get mutable reference to GuestHolonService".to_string(),
        )
    })?;
    service.register_internal_access(Arc::new(RefCell::new(nursery.clone())));

    // Step 4: Create the HolonSpaceManager with injected Nursery & HolonService
    let space_manager = Arc::new(HolonSpaceManager::new_with_nursery(
        guest_holon_service, // ✅ No need to clone, safe ownership transfer
        local_space_holon,
        ServiceRoutingPolicy::Combined,
        nursery, // ✅ Injected
    ));

    // Step 5: Wrap in `GuestHolonsContext` and return as a trait object
    Ok(Box::new(GuestHolonsContext::new(space_manager)))
}
