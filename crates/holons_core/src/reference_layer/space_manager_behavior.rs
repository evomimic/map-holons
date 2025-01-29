use crate::reference_layer::{HolonReference, HolonServiceApi, HolonStagingBehavior};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::nursery_access::NurseryAccess;
use crate::core_shared_objects::TransientCollection;
use crate::HolonCollectionApi;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

pub trait HolonSpaceBehavior {
    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    ///
    /// The `HolonSpaceManager` mediates access to holons by coordinating between
    /// the `local_cache_manager` for local requests and routing external requests.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess>;

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi>;

    /// Provides access to the nursery, creating it lazily if necessary.
    fn get_nursery_access(&self) -> Arc<RefCell<dyn NurseryAccess>>;

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Option<HolonReference>;

    /// Provides access to a component that supports the HolonStagingBehavior API
    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>>;

    /// Provides shared, thread-safe access to the transient state, initializing it lazily if necessary.
    ///
    /// The transient state is represented by a `TransientCollection`, which implements the `HolonCollectionApi` trait.
    /// This method ensures that the transient state is lazily initialized the first time it is accessed,
    /// and returns a shared reference to it as a `dyn HolonCollectionApi` trait object.
    ///
    /// # Behavior
    /// - If the `transient_state` is `None` (not yet initialized), this method will create a new
    ///   `TransientCollection` and store it in the `transient_state` field.
    /// - If the `transient_state` is already initialized, it will simply return a reference to the existing instance.
    ///
    /// # Thread Safety
    /// - The `transient_state` field is protected by a `Mutex` to ensure safe concurrent access in a
    ///   multi-threaded environment.
    /// - The `Arc` ensures that the transient state can be shared across threads without duplicating the underlying data.
    ///
    /// # Returns
    /// - An `Arc<dyn HolonCollectionApi>` that provides shared access to the transient state.
    ///
    ///
    /// # Errors
    /// - This method is not expected to fail under normal circumstances, as it guarantees the initialization
    ///   of the `transient_state` on-demand.
    fn get_transient_state(&self) -> Arc<Mutex<Option<TransientCollection>>>;
}
