use crate::core_shared_objects::cache_request_router::CacheRequestRouter;
use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, HolonCacheManager, Nursery, NurseryAccess, ServiceRoutingPolicy,
    TransientHolonManager, TransientManagerAccess,
};
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior, TransientHolonBehavior,
};
use crate::{HolonCollectionApi, TransientCollection};
use std::cell::RefCell;
use std::sync::Arc;

use std::fmt::{Debug, Formatter, Result};


pub struct HolonSpaceManager {
    /// Shared reference to the Holon service API (persists, retrieves, and queries holons).
    holon_service: Arc<dyn HolonServiceApi>,

    /// Optional reference to the space holon (authoritative context for other holons).
    local_holon_space: RefCell<Option<HolonReference>>,

    /// The Nursery manages **staged holons** for commit operations.
    nursery: Arc<RefCell<Nursery>>,

    /// Manages **transient holons** .
    transient_manager: Arc<RefCell<TransientHolonManager>>,

    /// Manages cache access for retrieving both local and external holons efficiently.
    cache_request_router: Arc<dyn HolonCacheAccess>,

    /// An ephemeral collection of references to staged or non-staged holons for temporary operations.
    transient_state: Arc<RefCell<TransientCollection>>,
}

impl HolonSpaceManager {
    /// Creates a new `HolonSpaceManager` from the given session data.
    ///
    /// This function initializes the `HolonSpaceManager` with:
    /// - A **pre-initialized Nursery** (empty if no staged holons exist).
    /// - A **pre-initialized TransientHolonManager** (empty if no transient holons exist).
    /// - A configured cache request router.
    ///
    /// # Parameters
    /// - `holon_service`: The holon service used for accessing and managing holons.
    /// - `local_holon_space`: An optional reference to the local holon space.
    /// - `cache_routing_policy`: Specifies how cache requests should be routed.
    /// - 'nursery': Initiliazed either empty or containing staged holons.
    /// - 'transient_manager': Initiliazed either empty or containing transient holons.
    ///
    /// # Returns
    /// A new instance of `HolonSpaceManager`
    pub fn new_with_managers(
        holon_service: Arc<dyn HolonServiceApi>,
        local_holon_space: Option<HolonReference>,
        cache_routing_policy: ServiceRoutingPolicy,
        nursery: Nursery,
        transient_manager: TransientHolonManager,
    ) -> Self {
        // Step 1: Initialize the Local Cache Manager
        let local_cache_manager = HolonCacheManager::new(Arc::clone(&holon_service));

        // Step 2: Create the CacheRequestRouter
        let cache_request_router: Arc<dyn HolonCacheAccess> =
            Arc::new(CacheRequestRouter::new(local_cache_manager, cache_routing_policy));

        // Step 3: Wrap the provided `Nursery` in an `Arc<RefCell<Nursery>>`
        let nursery_arc = Arc::new(RefCell::new(nursery));

        // Step 4: Wrap the provided `TransientHolonManager` in an `Arc<RefCell<TransientHolonManager>>`
        let transient_manager_arc = Arc::new(RefCell::new(transient_manager));

        // Step 5: Initialize and return the HolonSpaceManager
        Self {
            holon_service,
            local_holon_space: RefCell::new(local_holon_space),
            cache_request_router,
            nursery: nursery_arc,
            transient_manager: transient_manager_arc,
            transient_state: Arc::new(RefCell::new(TransientCollection::new())),
        }
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
        self.local_holon_space.borrow().clone()
    }

    /// Provides access to a component that supports the `HolonStagingBehavior` API.
    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>> {
        Arc::clone(&self.nursery) as Arc<RefCell<dyn HolonStagingBehavior>>
    }

    /// Provides access to a component that supports the `HolonStagingBehavior` API.
    fn get_transient_behavior_service(&self) -> Arc<RefCell<dyn TransientHolonBehavior>> {
        Arc::clone(&self.transient_manager) as Arc<RefCell<dyn TransientHolonBehavior>>
    }

    /// Provides access to the TransientHolonManager.
    fn get_transient_manager_access(&self) -> Arc<RefCell<dyn TransientManagerAccess>> {
        Arc::clone(&self.transient_manager) as Arc<RefCell<dyn TransientManagerAccess>>
    }

    /// Retrieves a shared reference to the transient state.
    fn get_transient_state(&self) -> Arc<RefCell<dyn HolonCollectionApi>> {
        Arc::clone(&self.transient_state) as Arc<RefCell<dyn HolonCollectionApi>>
    }

    /// Exports the staged holons from the nursery as a `SerializableHolonPool`.
    fn export_staged_holons(&self) -> SerializableHolonPool {
        self.nursery.borrow().export_staged_holons()
    }

    /// Exports the staged holons from the nursery as a `SerializableHolonPool`.
    fn export_transient_holons(&self) -> SerializableHolonPool {
        self.transient_manager.borrow().export_transient_holons()
    }

    /// Imports staged holons into the nursery from a `SerializableHolonPool`.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool) {
        self.nursery.borrow_mut().import_staged_holons(staged_holons);
    }

    /// Imports staged holons into the nursery from a `SerializableHolonPool`.
    fn import_transient_holons(&self, transient_holons: SerializableHolonPool) {
        self.transient_manager.borrow_mut().import_transient_holons(transient_holons);
    }

    fn set_space_holon(&self, holon: HolonReference) {
        *self.local_holon_space.borrow_mut() = Some(holon);
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
