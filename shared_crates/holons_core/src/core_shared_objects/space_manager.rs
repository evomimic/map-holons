use core_types::HolonError;

use crate::core_shared_objects::cache_request_router::CacheRequestRouter;
use crate::core_shared_objects::transactions::TransactionManager;
use crate::core_shared_objects::{HolonCacheAccess, HolonCacheManager, ServiceRoutingPolicy};
use crate::reference_layer::{HolonReference, HolonServiceApi, HolonSpaceBehavior};
use crate::TransientCollection;

use std::sync::{Arc, RwLock};

use crate::dances::dance_initiator::DanceInitiator;
use std::fmt::{Debug, Formatter};

pub struct HolonSpaceManager {
    /// Manages cache access for retrieving both local and external holons efficiently.
    cache_request_router: Arc<dyn HolonCacheAccess + Send + Sync>,

    /// Handles conductor dance calls.
    dance_initiator: Option<Arc<dyn DanceInitiator>>,

    /// Shared reference to the Holon service API (persists, retrieves, and queries holons).
    holon_service: Arc<dyn HolonServiceApi + Send + Sync>,

    /// Optional reference to the space holon (authoritative context for other holons).
    local_holon_space: RwLock<Option<HolonReference>>,

    /// An ephemeral collection of references to staged or non-staged holons for temporary operations.
    transient_state: Arc<RwLock<TransientCollection>>,

    /// Per-space transaction manager for opening and tracking transactions.
    transaction_manager: Arc<TransactionManager>,
}

impl HolonSpaceManager {
    /// Creates a new `HolonSpaceManager` from the given session data.
    ///
    /// This function initializes the `HolonSpaceManager` with:
    /// - A configured cache request router.
    ///
    /// # Parameters
    /// - `holon_service`: The holon service used for accessing and managing holons.
    /// - `local_holon_space`: An optional reference to the local holon space.
    /// - `cache_routing_policy`: Specifies how cache requests should be routed.
    /// # Returns
    /// A new instance of `HolonSpaceManager`
    pub fn new_with_managers(
        dance_initiator: Option<Arc<dyn DanceInitiator>>,
        holon_service: Arc<dyn HolonServiceApi>,
        local_holon_space: Option<HolonReference>,
        cache_routing_policy: ServiceRoutingPolicy,
    ) -> Self {
        // Step 1: Initialize the Local Cache Manager inside Arc<RwLock>
        let local_cache_manager =
            Arc::new(RwLock::new(HolonCacheManager::new(Arc::clone(&holon_service))));

        // Step 2: Create the CacheRequestRouter using thread-safe manager
        let cache_request_router: Arc<dyn HolonCacheAccess> = Arc::new(CacheRequestRouter::new(
            Arc::clone(&local_cache_manager),
            cache_routing_policy,
        ));

        // Step 3: Initialize the per-space transaction manager.
        let transaction_manager = Arc::new(TransactionManager::new());

        // Step 4: Initialize and return the HolonSpaceManager with thread-safe fields
        Self {
            cache_request_router,
            dance_initiator,
            holon_service,
            local_holon_space: RwLock::new(local_holon_space),
            transient_state: Arc::new(RwLock::new(TransientCollection::new())),
            transaction_manager,
        }
    }

    /// Provides access to the per-space transaction manager.
    pub fn get_transaction_manager(&self) -> Arc<TransactionManager> {
        // Step 1: Clone the Arc for the caller.
        Arc::clone(&self.transaction_manager)
    }
}

impl HolonSpaceBehavior for HolonSpaceManager {
    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        Arc::clone(&self.cache_request_router)
    }

    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError> {
        self.dance_initiator
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| HolonError::ServiceNotAvailable("DanceInitiator".into()))
    }

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync> {
        Arc::clone(&self.holon_service)
    }

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError> {
        let guard = self.local_holon_space.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on local_holon_space: {}",
                e
            ))
        })?;

        Ok(guard.clone())
    }

    /// Retrieves a shared reference to the transient state.
    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>> {
        Arc::clone(&self.transient_state)
    }

    /// Updates the local space holon reference.
    fn set_space_holon(&self, holon: HolonReference) -> Result<(), HolonError> {
        let mut guard = self.local_holon_space.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on local_holon_space: {}",
                e
            ))
        })?;

        *guard = Some(holon);
        Ok(())
    }

    /// Provides access to the per-space transaction manager.
    fn get_transaction_manager(&self) -> Arc<TransactionManager> {
        Arc::clone(&self.transaction_manager)
    }
}

impl Debug for HolonSpaceManager {
    /// Implements custom `Debug` formatting for `HolonSpaceManager`.
    ///
    /// This method avoids printing non-essential internals to keep logs readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolonSpaceManager")
            .field("holon_service", &"<HolonServiceApi>")
            .field("local_holon_space", &self.local_holon_space)
            .field("cache_request_router", &"<CacheRequestRouter>")
            .field("transient_state", &"<TransientCollection>")
            .field("transaction_manager", &"<TransactionManager>")
            .finish()
    }
}
