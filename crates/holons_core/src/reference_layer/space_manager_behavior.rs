use crate::reference_layer::{HolonReference, HolonServiceApi, HolonStagingBehavior};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::nursery_access::NurseryAccess;
use std::cell::RefCell;
use std::sync::Arc;

pub trait HolonSpaceBehavior {
    /// Provides access to the cache via a reference to an implementer of `HolonCacheAccess`.
    ///
    /// The `HolonSpaceManager` mediates access to holons by coordinating between
    /// the `local_cache_manager` for local requests and routing external requests.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess>;

    /// Provides access to the Holon service API.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi>;

    /// Provides access to a component that supports the HolonStagingBehavior API
    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>>;

    /// Provides access to the nursery, creating it lazily if necessary.
    fn get_nursery_access(&self) -> Arc<RefCell<dyn NurseryAccess>>;

    /// Retrieves a reference to the space holon if it exists.
    fn get_space_holon(&self) -> Option<HolonReference>;
}
