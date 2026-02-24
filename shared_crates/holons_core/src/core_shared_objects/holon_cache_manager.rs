use std::sync::{Arc, RwLock};
use tracing::debug;

use super::{holon_cache::HolonCache, Holon};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::HolonServiceApi;
use crate::{HolonCacheAccess, HolonCollection, RelationshipCache, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Debug)]
pub struct HolonCacheManager {
    cache: HolonCache, // Thread-safe cache of holons
    relationship_cache: RwLock<RelationshipCache>,
    holon_service: Arc<dyn HolonServiceApi>,
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(holon_service: Arc<dyn HolonServiceApi>) -> Self {
        Self {
            cache: HolonCache::new(),
            relationship_cache: RwLock::new(RelationshipCache::new()),
            holon_service,
        }
    }
}

impl HolonCacheAccess for HolonCacheManager {
    /// Retrieves a Holon by its `HolonId`.
    /// - If the Holon is already in the cache, it returns the cached reference.
    /// - Otherwise, it fetches the Holon using the HolonService, adds it to the cache, and returns it.
    /// The behavior of this method is different on the client-side, where all Holons are cached
    /// in a single cache, and the guest-side where each space has its own cache.
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Attempt to retrieve the holon from the cache
        if let Some(cached) = self.cache.get(holon_id) {
            debug!("Holon {:?} retrieved from cache.", holon_id);
            return Ok(cached.clone());
        }
        use tracing::warn;
        // If not found, resolve it from the HolonService
        warn!("Holon with HolonId {:?} not in cache. Fetching using HolonService.", holon_id);
        let holon = self.holon_service.fetch_holon_internal(holon_id)?;
        let arc_holon = Arc::new(RwLock::new(holon));

        // Insert the resolved holon into the cache
        self.cache.insert(holon_id.clone(), arc_holon.clone());
        debug!("Holon with HolonId {:?} fetched and cached.", holon_id);

        Ok(arc_holon)
    }

    fn get_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.relationship_cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {}", e))
            })?
            .related_holons(
                context,
                self.holon_service.as_ref(),
                source_holon_id,
                relationship_name,
            )
    }

    fn get_all_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        self.relationship_cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {}", e))
            })?
            .get_all_related_holons(context, self.holon_service.as_ref(), source_holon_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*; // brings HolonCacheManager into scope

    // Generic helper to assert Send + Sync at compile time
    fn assert_thread_safe<T: Send + Sync>() {}

    #[test]
    fn assert_cache_manager_is_thread_safe() {
        assert_thread_safe::<HolonCacheManager>();
    }
}
