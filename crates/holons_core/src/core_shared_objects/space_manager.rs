use crate::core_shared_objects::cache_request_router::CacheRequestRouter;
use crate::core_shared_objects::{
    Holon, HolonCacheAccess, HolonCacheManager, Nursery, NurseryAccess, ServiceRoutingPolicy,
};
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
};
use shared_types_holon::MapString;
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::TransientCollection;
use crate::HolonCollectionApi;

#[derive(Debug)]
pub struct HolonSpaceManager {
    pub cache_request_router: Arc<dyn HolonCacheAccess>, // Shared CacheRequestRouter
    pub holon_service: Arc<dyn HolonServiceApi>,         // Shared Holon service
    pub local_holon_space: Option<HolonReference>,       // Optional reference to the space holon
    pub nursery: RefCell<Option<Nursery>>,               // Lazily initialized nursery
    // Arc: Allows safe sharing of transient_state across threads.
    // Mutex: Provides interior mutability and ensures thread-safe access.
    // Option: Allows lazy initialization of transient_state.
    transient_state: Arc<Mutex<Option<TransientCollection>>>,
}

impl HolonSpaceManager {
    /// Private helper function that ensures the nursery is initialized. If it is uninitialized,
    /// this function will create a new Nursery instance with empty staged holons and keyed index.
    ///
    /// Returns a reference to the initialized nursery.
    fn ensure_nursery_initialized(&self) -> Ref<Option<Nursery>> {
        if self.nursery.borrow().is_none() {
            // Replace with actual logic for staged holons and keyed index
            let staged_holons = vec![];
            let keyed_index = BTreeMap::new();

            // Initialize the nursery
            *self.nursery.borrow_mut() =
                Some(Nursery::new_from_staged_holons(staged_holons, keyed_index));
        }

        // Return a reference to the initialized nursery
        self.nursery.borrow()
    }

    /// Creates a new `HolonSpaceManager` from the given session data, initializing its fields,
    /// and optionally populating the `nursery` based on the provided `staged_holons`.
    ///
    /// # Parameters
    /// - `holon_service`: The holon service to be owned by the `HolonSpaceManager`.
    /// - `staged_holons`: A collection of holons to populate the nursery. If empty, the nursery starts uninitialized.
    /// - `keyed_index`: A keyed index used to construct the nursery if `staged_holons` is not empty.
    /// - `space_holon_ref`: An optional reference to the local holon space.
    /// - `cache_routing_policy`: The routing policy for cache interactions.
    ///
    /// # Returns
    /// A new instance of `HolonSpaceManager`.
    pub fn new_from_session(
        cache_routing_policy: ServiceRoutingPolicy,
        holon_service: Arc<dyn HolonServiceApi>,
        keyed_index: BTreeMap<MapString, usize>,
        space_holon_ref: Option<HolonReference>,
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        transient_state: Arc<Mutex<Option<TransientCollection>>>,
    ) -> Self {
        // Initialize the nursery
        let nursery = RefCell::new(if staged_holons.is_empty() {
            None
        } else {
            Some(Nursery::new_from_staged_holons(staged_holons, keyed_index))
        });

        // Initialize the local cache manager
        let local_cache_manager = HolonCacheManager::new(Arc::clone(&holon_service));

        // Step 3: Create the CacheRequestRouter and wrap it in an Arc
        let cache_request_router: Arc<dyn HolonCacheAccess> = Arc::new(CacheRequestRouter::new(
            local_cache_manager,
            cache_routing_policy,
            //outbound_proxies,
        ));

        // Step 4: Construct and return the HolonSpaceManager
        HolonSpaceManager {
            cache_request_router,
            holon_service,
            local_holon_space: space_holon_ref,
            nursery,
            transient_state,
        }
    }

    pub fn get_transient_state(&self) -> Arc<Mutex<Option<TransientCollection>>> {
        self.transient_state.clone()
    }

    pub fn set_space_holon(&mut self, space: HolonReference) {
        self.local_holon_space = Some(space);
    }
}

impl HolonSpaceBehavior for HolonSpaceManager {
    /// Returns an `Arc<dyn HolonCacheAccess>` wrapping the `HolonSpaceManager`.
    ///
    /// This allows the `HolonSpaceManager` to expose itself as a cache access service while
    /// mediating access between local and external holon requests.
    /// Returns a reference to the `CacheRequestRouter`.
    ///
    /// This method exposes the `HolonCacheAccess` functionality via the router.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess> {
        Arc::clone(&self.cache_request_router)
    }
    // fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess> {
    //     // Wrap `self` in an Arc for shared ownership
    //     Arc::new(self.clone()) as Arc<dyn HolonCacheAccess>
    // }

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi> {
        Arc::clone(&self.holon_service)
    }

    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>> {
        // Ensure the nursery is initialized and get a reference to it
        let nursery_borrow = self.ensure_nursery_initialized();

        // Unwrap the initialized Nursery (it must exist at this point) and wrap it as a trait object
        Arc::new(RefCell::new(
            nursery_borrow.as_ref().expect("Nursery should have been initialized").clone(),
        ))
    }

    /// Provides access to the nursery, creating it lazily if necessary.
    ///
    /// Lazily initializes the `nursery` if it has not been initialized yet. The `nursery`
    /// is returned as a shared `Arc<RefCell<dyn NurseryAccess>>`.
    fn get_nursery_access(&self) -> Arc<RefCell<dyn NurseryAccess>> {
        // Ensure the nursery is initialized and get a reference to it
        let nursery_borrow = self.ensure_nursery_initialized();

        // Unwrap the initialized Nursery (it must exist at this point) and wrap it as a trait object
        Arc::new(RefCell::new(
            nursery_borrow.as_ref().expect("Nursery should have been initialized").clone(),
        ))
    }

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Option<HolonReference> {
        self.local_holon_space.clone()
    }

    /// Retrieves a shared, thread-save reference to the transient_state.
    fn get_transient_state(&self) -> Arc<Mutex<Option<TransientCollection>>> {
        self.get_transient_state()
    }
}
