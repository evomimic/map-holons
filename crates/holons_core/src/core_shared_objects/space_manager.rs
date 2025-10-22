use core_types::HolonError;

use crate::core_shared_objects::cache_request_router::CacheRequestRouter;
use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, HolonCacheManager, Nursery, ServiceRoutingPolicy, TransientHolonManager,
    TransientManagerAccess,
};
use crate::dances::DanceCallServiceApi;
use crate::reference_layer::{HolonReference, HolonServiceApi, HolonSpaceBehavior};
use crate::{HolonStagingBehavior, NurseryAccess, TransientCollection, TransientHolonBehavior};
use std::sync::{Arc, RwLock};

use std::fmt::{Debug, Formatter};

pub struct HolonSpaceManager {
    /// Manages cache access for retrieving both local and external holons efficiently.
    cache_request_router: Arc<dyn HolonCacheAccess + Send + Sync>,

    /// Handles conductor dance calls.
    dance_call_service: Arc<dyn DanceCallServiceApi>,

    /// Shared reference to the Holon service API (persists, retrieves, and queries holons).
    holon_service: Arc<dyn HolonServiceApi + Send + Sync>,

    /// Optional reference to the space holon (authoritative context for other holons).
    local_holon_space: RwLock<Option<HolonReference>>,

    /// The Nursery manages **staged holons** for commit operations.
    nursery: Arc<RwLock<Nursery>>,

    /// Manages **transient holons** .
    transient_manager: Arc<RwLock<TransientHolonManager>>,

    /// An ephemeral collection of references to staged or non-staged holons for temporary operations.
    transient_state: Arc<RwLock<TransientCollection>>,
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
        dance_call_service: Arc<dyn DanceCallServiceApi>,
        holon_service: Arc<dyn HolonServiceApi>,
        local_holon_space: Option<HolonReference>,
        cache_routing_policy: ServiceRoutingPolicy,
        nursery: Nursery,
        transient_manager: TransientHolonManager,
    ) -> Self {
        // Step 1: Initialize the Local Cache Manager inside Arc<RwLock>
        let local_cache_manager =
            Arc::new(RwLock::new(HolonCacheManager::new(Arc::clone(&holon_service))));

        // Step 2: Create the CacheRequestRouter using thread-safe manager
        let cache_request_router: Arc<dyn HolonCacheAccess> = Arc::new(CacheRequestRouter::new(
            Arc::clone(&local_cache_manager),
            cache_routing_policy,
        ));

        // Step 3: Wrap the provided `Nursery` in an `Arc<RwLock<Nursery>>`
        let nursery_lock = Arc::new(RwLock::new(nursery));

        // Step 4: Wrap the provided `TransientHolonManager` in an `Arc<RwLock<TransientHolonManager>>`
        let transient_lock = Arc::new(RwLock::new(transient_manager));

        // Step 5: Initialize and return the HolonSpaceManager with thread-safe fields
        Self {
            cache_request_router,
            dance_call_service,
            holon_service,
            local_holon_space: RwLock::new(local_holon_space),
            nursery: nursery_lock,
            transient_manager: transient_lock,
            transient_state: Arc::new(RwLock::new(TransientCollection::new())),
        }
    }
}

impl HolonSpaceBehavior for HolonSpaceManager {
    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        Arc::clone(&self.cache_request_router)
    }

    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    fn get_dance_call_service(&self) -> Arc<dyn DanceCallServiceApi> {
        Arc::clone(&self.dance_call_service)
    }

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync> {
        Arc::clone(&self.holon_service)
    }

    /// Provides access to the nursery.
    fn get_nursery_access(&self) -> Arc<RwLock<dyn NurseryAccess + Send + Sync>> {
        Arc::clone(&self.nursery) as Arc<RwLock<dyn NurseryAccess + Send + Sync>>
    }

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Option<HolonReference> {
        self.local_holon_space
            .read()
            .expect("Failed to acquire read lock on local_holon_space")
            .clone()
    }

    /// Provides access to a component that supports the `HolonStagingBehavior` API.
    fn get_staging_service(&self) -> Arc<RwLock<dyn HolonStagingBehavior + Send + Sync>> {
        Arc::clone(&self.nursery) as Arc<RwLock<dyn HolonStagingBehavior + Send + Sync>>
    }

    /// Provides access to a component that supports the `HolonStagingBehavior` API.
    fn get_transient_behavior_service(
        &self,
    ) -> Arc<RwLock<dyn TransientHolonBehavior + Send + Sync>> {
        Arc::clone(&self.transient_manager) as Arc<RwLock<dyn TransientHolonBehavior + Send + Sync>>
    }

    /// Provides access to the TransientHolonManager.
    fn get_transient_manager_access(
        &self,
    ) -> Arc<RwLock<dyn TransientManagerAccess + Send + Sync>> {
        Arc::clone(&self.transient_manager) as Arc<RwLock<dyn TransientManagerAccess + Send + Sync>>
    }

    /// Retrieves a shared reference to the transient state.
    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>> {
        Arc::clone(&self.transient_state)
    }

    /// Exports the staged holons from the nursery as a `SerializableHolonPool`.
    fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.nursery
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on nursery: {}",
                    e
                ))
            })?
            .export_staged_holons()
    }

    /// Exports the staged holons from the nursery as a `SerializableHolonPool`.
    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.transient_manager
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on transient_manager: {}",
                    e
                ))
            })?
            .export_transient_holons()
    }

    /// Imports staged holons into the nursery from a `SerializableHolonPool`.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool) {
        self.nursery
            .write()
            .expect("Failed to acquire write lock on nursery")
            .import_staged_holons(staged_holons);
    }

    /// Imports staged holons into the nursery from a `SerializableHolonPool`.
    fn import_transient_holons(&self, transient_holons: SerializableHolonPool) {
        self.transient_manager
            .write()
            .expect("Failed to acquire write lock on transient_manager")
            .import_transient_holons(transient_holons);
    }

    fn set_space_holon(&self, holon: HolonReference) {
        *self
            .local_holon_space
            .write()
            .expect("Failed to acquire write lock on local_holon_space") = Some(holon);
    }
}
impl Debug for HolonSpaceManager {
    /// Implements custom `Debug` formatting for `HolonSpaceManager`.
    ///
    /// This method ensures that the `internal_nursery_access` field is **not printed** to avoid
    /// redundant logging, as it holds a **second reference** to the same `Nursery` instance.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
