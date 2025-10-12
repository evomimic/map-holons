use std::sync::{Arc, RwLock};
use tracing::debug;

use super::{holon_cache::HolonCache, Holon};
use crate::reference_layer::HolonServiceApi;
use crate::{
    HolonCacheAccess, HolonCollection, HolonsContextBehavior, RelationshipCache, RelationshipMap,
};
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Debug)]
pub struct HolonCacheManager {
    cache: RwLock<HolonCache>, // Thread-safe cache of holons
    relationship_cache: RwLock<RelationshipCache>,
    holon_service: Arc<dyn HolonServiceApi>,
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(holon_service: Arc<dyn HolonServiceApi>) -> Self {
        Self {
            cache: RwLock::new(HolonCache::new()),
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
        // Read lock the cache to check for existing holon
        if let Some(cached) = self
            .cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {}", e))
            })?
            .get(holon_id)
        {
            debug!("Holon {:?} retrieved from cache.", holon_id);
            return Ok(cached.clone());
        }

        // If not in cache, resolve the Holon using the resolver
        debug!("Holon with HolonId {:?} not in cache. Fetching using HolonSpace.", holon_id);
        let holon = self.holon_service.fetch_holon_internal(holon_id)?;

        let arc_holon = Arc::new(RwLock::new(holon));

        // Insert the Holon into the cache using a mutable borrow
        self.cache
            .write()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache write lock poisoned: {}", e))
            })?
            .insert(holon_id.clone(), arc_holon.clone());
        debug!("Holon with LocalId {:?} fetched and cached.", holon_id);
        Ok(arc_holon)
    }

    fn get_related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.relationship_cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {}", e))
            })?
            .get_related_holons(self.holon_service.as_ref(), source_holon_id, relationship_name)
    }

    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
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
