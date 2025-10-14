use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::debug;

use crate::core_shared_objects::{HolonCollection, RelationshipMap};
use crate::reference_layer::HolonServiceApi;
use crate::HolonsContextBehavior;
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Clone, Debug)]
pub struct RelationshipCache {
    cache: Arc<RwLock<HashMap<HolonId, RelationshipMap>>>,
}

// TODO: Consider replacing `HashMap<HolonId, RelationshipMap>` with a fine-grained
// cache keyed by (HolonId, RelationshipName), e.g. `Cache<(HolonId, RelationshipName), Arc<RwLock<HolonCollection>>>`.
// This would enable lock-free reads and concurrent inserts at the relationship level,
// eliminating the need for outer RwLock and improving cache concurrency.
impl RelationshipCache {
    /// Creates a new RelationshipCache with an empty cache.
    pub fn new() -> Self {
        Self { cache: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Retrieves a RelationshipMap for the source HolonReference by calling the HolonService to fetch all related Holons.
    pub fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        holon_service.fetch_all_related_holons_internal(context, source_holon_id)
    }

    /// Retrieves the `HolonCollection` containing references to all holons that are related
    /// to the specified `source_holon_id` via the specified `relationship_name` Note
    /// that the `HolonCollection` could be empty.
    ///
    /// If the relationship data for the `source_holon_id` and `relationship_name` is already in
    /// the cache, it is returned immediately. Otherwise, it is fetched from the `HolonServiceApi`,
    /// inserted into the cache, and then returned.
    ///
    /// This implementation supports both **lazy loading** and **exactly-once** semantics. The first
    /// time a cache miss occurs for a `source_holon_id`, an entry for that `source_holon_id` is added
    /// to the cache, along with an entry for the requested `relationship_name`. If there are no
    /// target holons for the requested relationship, an empty `HolonCollection` is cached, avoiding
    /// repeated calls to the `fetch_related_holons` method of the `HolonServiceApi`.
    pub fn get_related_holons(
        &self,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        // First, check if the relationship exists in the cache (immutable borrow)
        {
            let cache = self.cache.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on relationship_cache: {}",
                    e
                ))
            })?;
            if let Some(relationship_map) = cache.get(&source_holon_id) {
                if let Some(related_holons) =
                    relationship_map.get_collection_for_relationship(relationship_name)
                {
                    // Cache hit: return the cached HolonCollection
                    debug!(
                        "Cache hit for source_holon_id: {:?}, relationship_name: {:?}",
                        source_holon_id, relationship_name
                    );
                    return Ok(Arc::clone(&related_holons));
                }
            }
        } // Immutable borrow ends here

        // Cache miss: Fetch related holons from the HolonServiceApi
        debug!(
        "Cache miss for source_holon_id: {:?}, relationship_name: {:?}. Fetching from HolonServiceApi.",
        source_holon_id, relationship_name
    );
        let fetched_holons =
            holon_service.fetch_related_holons_internal(&source_holon_id, relationship_name)?;
        // Wrap in Arc<RwLock> for caching
        let fetched_arc = Arc::new(RwLock::new(fetched_holons));

        // Update the cache
        {
            let mut cache = self.cache.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on relationship_cache: {}",
                    e
                ))
            })?;
            let relationship_map =
                cache.entry(source_holon_id.clone()).or_insert_with(RelationshipMap::new_empty);
            relationship_map.insert(relationship_name.clone(), Arc::clone(&fetched_arc));
        }
        // Return the fetched holons
        Ok(fetched_arc)
    }
}
