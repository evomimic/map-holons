use crate::guest_shared_objects::GuestHolonService;
use core_types::{HolonError, HolonId};
use holons_core::{
    core_shared_objects::{
        holon_pool::SerializableHolonPool,
        space_manager::HolonSpaceManager,
        transactions::{TransactionContext, TxId},
        ServiceRoutingPolicy,
    },
    HolonServiceApi,
};
use std::sync::{Arc, RwLock};
use tracing::{
    info,
    // warn,
};

/// Initializes a new guest-side context with a `HolonSpaceManager` configured for Holochain execution.
///
/// This function sets up:
/// - A `GuestHolonService` as the persistence and retrieval layer.
/// - A space manager configured with **guest-specific routing policies**.
/// - An implicit transaction opened via the per-space `TransactionManager`.
/// - Internal nursery access, required for commit operations.
///
/// This function also ensures that a HolonSpace Holon exists in the local DHT.
///
/// # Arguments
/// * `transient_holons` - The `SerializableHolonPool` containing transient holons from the session_state state.
/// * `staged_holons` - The `SerializableHolonPool` containing staged holons from the session_state state.
/// * `local_space_holon` - An optional `HolonId` to anchor the local holon space (must be saved).
///
/// # Returns
/// * `Ok(Arc<TransactionContext>)` - The initialized guest context if successful.
/// * `Err(HolonError)` - If opening the default transaction fails.
///
/// # Errors
/// This function returns an error if it fails to ensure that a **HolonSpace Holon** exists.
/// Errors may occur if:
/// - The DHT lookup for the HolonSpace Holon fails.
/// - There are issues retrieving holons from persistent storage.
/// - The creation of a new HolonSpace Holon encounters a failure.
pub fn init_guest_context(
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_space_holon_id: Option<HolonId>,
    tx_id: TxId,
) -> Result<Arc<TransactionContext>, HolonError> {
    info!("\n ========== Initializing GUEST CONTEXT ============");

    // Step 1: Create the GuestHolonService (keep a concrete handle for registration).
    let guest_holon_service_concrete = Arc::new(GuestHolonService::new());

    // Step 1b: Also expose it as the HolonServiceApi trait object for the space manager.
    let guest_holon_service: Arc<dyn HolonServiceApi + Send + Sync> =
        guest_holon_service_concrete.clone();

    // Step 2: Create the HolonSpaceManager with guest routing policy.
    //
    // NOTE: `local_space_holon_id` may be `None` if the caller did not provide one
    // in SessionState; in that case the guest will ensure/create it later during ingress.
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        guest_holon_service,
        local_space_holon_id,
        ServiceRoutingPolicy::Combined,
    ));

    // Step 3: Open the default transaction for this space.
    let transaction_context = space_manager
        .get_transaction_manager()
        .open_transaction_with_id(Arc::clone(&space_manager), tx_id)?;

    // Step 4: Load staged and transient holons into the transaction.
    transaction_context.import_staged_holons(staged_holons)?;
    transaction_context.import_transient_holons(transient_holons)?;

    // Step 5: Register internal nursery access for commit.
    let nursery_for_internal_access = transaction_context.nursery();
    guest_holon_service_concrete.register_internal_access(Arc::new(RwLock::new(
        nursery_for_internal_access.as_ref().clone(),
    )));

    // Step 6: Return the transaction context directly.
    Ok(transaction_context)
}
