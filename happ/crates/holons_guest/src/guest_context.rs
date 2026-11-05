use crate::guest_shared_objects::GuestHolonService;
use core_types::{HolonError, HolonId};
use holons_boundary::session_state::SerializableHolonPool;
use holons_core::core_shared_objects::descriptor_registry::EffectiveHolonDescriptorRegistry;
use holons_core::{
    core_shared_objects::{
        space_manager::HolonSpaceManager,
        transactions::{TransactionContext, TxId},
        ServiceRoutingPolicy,
    },
    HolonServiceApi,
};
use std::sync::Arc;

/// Initializes a new guest-side context with a `HolonSpaceManager` configured for Holochain execution.
///
/// This function sets up:
/// - A `GuestHolonService` as the persistence and retrieval layer.
/// - A space manager configured with **guest-specific routing policies**.
/// - An implicit transaction opened via the per-space `TransactionManager`.
///
/// This function deliberately does not create a local HolonSpace. Ordinary
/// transactions must arrive with an existing local space; the dance adapter
/// permits an explicitly marked bootstrap-provisioning transaction to remain
/// unbound until the CoreSchemaSpace has been committed.
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
/// This function returns an error if it cannot create or hydrate the transaction
/// context from the transport session state.
pub fn init_guest_context(
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_space_holon_id: Option<HolonId>,
    tx_id: TxId,
    restricted_cache_read: bool,
) -> Result<Arc<TransactionContext>, HolonError> {
    Ok(init_guest_context_with_registry(
        transient_holons,
        staged_holons,
        local_space_holon_id,
        tx_id,
        restricted_cache_read,
        None,
        false,
    )?
    .0)
}

/// Restore the registry before binding agent pools that may reference its published results.
/// The returned strong owner is retained by the request, since the space association is weak.
pub(crate) fn init_guest_context_with_registry(
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_space_holon_id: Option<HolonId>,
    tx_id: TxId,
    restricted_cache_read: bool,
    registry: Option<holons_boundary::DescriptorRegistryWire>,
    profile_commit: bool,
) -> Result<(Arc<TransactionContext>, Option<Arc<EffectiveHolonDescriptorRegistry>>), HolonError> {
    // Step 1: Create the GuestHolonService and expose it as HolonServiceApi.
    let guest_holon_service: Arc<dyn HolonServiceApi + Send + Sync> =
        Arc::new(GuestHolonService::new());

    // Step 2: Create the HolonSpaceManager with guest routing policy.
    //
    // A missing local-space id is represented faithfully here. The dance adapter
    // rejects it for ordinary transactions and accepts it only for the
    // host-authorized bootstrap-provisioning transaction.
    let space_manager = HolonSpaceManager::new_with_managers(
        None,
        guest_holon_service,
        local_space_holon_id,
        ServiceRoutingPolicy::Combined,
    );

    // Step 3: Open the default transaction for this space.
    let transaction_context = if restricted_cache_read {
        space_manager
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction_with_id(
                Arc::clone(&space_manager),
                tx_id,
            )?
    } else {
        space_manager
            .get_transaction_manager()
            .open_public_transaction_with_id(Arc::clone(&space_manager), tx_id)?
    };

    let profile = profile_commit && cfg!(feature = "commit-performance-profiling");
    let started = profile.then(crate::performance::timestamp_micros).flatten();
    let probe = holons_core::core_shared_objects::read_observations::CacheReadProbe::start(profile);
    let mut index_started = None;
    let mut index_us = 0;
    let registry = match registry {
        Some(wire) => wire.restore_with_observer(&transaction_context, &mut |phase| {
            if profile {
                if phase == "IndexRebuild" {
                    index_started = crate::performance::timestamp_micros();
                }
                if phase == "Complete" {
                    if let (Some(start), Some(end)) =
                        (index_started, crate::performance::timestamp_micros())
                    {
                        index_us = end.saturating_sub(start);
                    }
                }
            }
        })?,
        None => {
            // Cold requests can prepare saved products too. Retain their ordinary
            // registry for this request without inventing a validation-owned cache.
            let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space_manager));
            space_manager.attach_descriptor_registry(&registry);
            registry
        }
    };
    let reads =
        holons_core::core_shared_objects::read_observations::CacheReadSummary::from_observations(
            &probe.finish(),
        );
    if let (Some(start), Some(end)) =
        (started, profile.then(crate::performance::timestamp_micros).flatten())
    {
        let (holon_types, derived) = registry.population()?;
        hdk::prelude::info!("[commit-registry-ingress] restoration_us={} index_rebuild_us={} holon_types={} derived={} logical_reads={} cache_misses={}",
            end.saturating_sub(start).saturating_sub(index_us), index_us, holon_types, derived, reads.requests, reads.misses);
    }

    // Step 4: Bind agent pools only after the registry owner is discoverable.
    let bound_staged_holons = staged_holons.bind(&transaction_context)?;
    let bound_transient_holons = transient_holons.bind(&transaction_context)?;

    transaction_context.import_staged_holons(bound_staged_holons)?;
    transaction_context.import_transient_holons(bound_transient_holons)?;

    // Retain the registry alongside the agent context for this request.
    Ok((transaction_context, Some(registry)))
}
