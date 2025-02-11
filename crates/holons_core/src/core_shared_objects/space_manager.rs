use super::TransientCollection;
use crate::core_shared_objects::cache_request_router::CacheRequestRouter;
use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, HolonCacheManager, Nursery, NurseryAccess, ServiceRoutingPolicy,
};
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
};
use crate::HolonCollectionApi;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter, Result};
use std::sync::Arc;

pub struct HolonSpaceManager {
    /// Shared reference to the Holon service API (persists, retrieves, and queries holons).
    pub holon_service: Arc<dyn HolonServiceApi>,

    /// Optional reference to the space holon (authoritative context for other holons).
    pub local_holon_space: Option<HolonReference>,

    /// The Nursery manages **staged holons** for commit operations.
    pub nursery: Arc<RefCell<Nursery>>,

    /// Manages cache access for retrieving both local and external holons efficiently.
    pub cache_request_router: Arc<dyn HolonCacheAccess>,

    /// An ephemeral collection of references to staged or non-staged holons for temporary operations.
    pub transient_state: Arc<RefCell<TransientCollection>>,
}

impl HolonSpaceManager {
    /// Creates a new `HolonSpaceManager` from the given session data.
    ///
    /// This function initializes the `HolonSpaceManager` with:
    /// - A **pre-initialized Nursery** (empty if no staged holons exist).
    /// - A configured cache request router.
    /// - A transient state container (initialized as an empty collection).
    /// - (Optional) Internal access to the Nursery for privileged services.
    ///
    /// # Parameters
    /// - `holon_service`: The holon service used for accessing and managing holons.
    /// - `staged_holons`: A `SerializableHolonPool` containing staged holons for the nursery.
    /// - `local_holon_space`: An optional reference to the local holon space.
    /// - `cache_routing_policy`: Specifies how cache requests should be routed.
    /// - `internal_nursery_access`: (Optional) Grants privileged access to `NurseryAccessInternal`
    ///   for services that require it (e.g., `GuestHolonService`).
    ///
    /// # Returns
    /// A new instance of `HolonSpaceManager`, with or without internal nursery access
    /// depending on the provided parameters.
    pub fn new_with_nursery(
        holon_service: Arc<dyn HolonServiceApi>, // ✅ Injected Holon Service
        local_holon_space: Option<HolonReference>,
        cache_routing_policy: ServiceRoutingPolicy,
        nursery: Nursery, // ✅ Injected, already initialized
    ) -> Self {
        // Step 1: Initialize the Local Cache Manager
        let local_cache_manager = HolonCacheManager::new(Arc::clone(&holon_service));

        // Step 2: Create the CacheRequestRouter
        let cache_request_router: Arc<dyn HolonCacheAccess> =
            Arc::new(CacheRequestRouter::new(local_cache_manager, cache_routing_policy));

        // Step 3: Wrap the provided `Nursery` in an `Arc<RefCell<Nursery>>`
        let nursery_arc = Arc::new(RefCell::new(nursery));

        // Step 4: Initialize and return the HolonSpaceManager
        Self {
            holon_service,
            local_holon_space,
            nursery: nursery_arc,
            cache_request_router,
            transient_state: Arc::new(RefCell::new(TransientCollection::new())),
        }
    }

    /// Updates the local space holon reference.
    ///
    /// # Arguments
    /// - `space` - The new `HolonReference` for the space.
    pub fn set_space_holon(&mut self, space: HolonReference) {
        self.local_holon_space = Some(space);
    }
}

impl HolonSpaceBehavior for HolonSpaceManager {
    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess> {
        Arc::clone(&self.cache_request_router)
    }

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi> {
        Arc::clone(&self.holon_service)
    }

    /// Provides access to the nursery.
    fn get_nursery_access(&self) -> Arc<RefCell<dyn NurseryAccess>> {
        Arc::clone(&self.nursery) as Arc<RefCell<dyn NurseryAccess>>
    }

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Option<HolonReference> {
        self.local_holon_space.clone()
    }

    /// Provides access to a component that supports the `HolonStagingBehavior` API.
    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>> {
        Arc::clone(&self.nursery) as Arc<RefCell<dyn HolonStagingBehavior>>
    }

    /// Exports the staged holons from the nursery as a `SerializableHolonPool`.
    fn export_staged_holons(&self) -> SerializableHolonPool {
        self.nursery.borrow().export_staged_holons()
    }

    /// Imports staged holons into the nursery from a `SerializableHolonPool`.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool) {
        self.nursery.borrow_mut().import_staged_holons(staged_holons);
    }

    /// Retrieves a shared reference to the transient state.
    fn get_transient_state(&self) -> Arc<RefCell<dyn HolonCollectionApi>> {
        Arc::clone(&self.transient_state) as Arc<RefCell<dyn HolonCollectionApi>>
    }
}
impl Debug for HolonSpaceManager {
    /// Implements custom `Debug` formatting for `HolonSpaceManager`.
    ///
    /// This method ensures that the `internal_nursery_access` field is **not printed** to avoid
    /// redundant logging, as it holds a **second reference** to the same `Nursery` instance.
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("HolonSpaceManager")
            .field("holon_service", &"<HolonServiceApi>")
            .field("local_holon_space", &self.local_holon_space)
            .field("nursery", &self.nursery) // ✅ Print only once
            .field("cache_request_router", &"<CacheRequestRouter>")
            .field("transient_state", &"<TransientCollection>")
            .field("internal_nursery_access", &"Hidden") // ✅ Avoid duplicate printing
            .finish()
    }
}
