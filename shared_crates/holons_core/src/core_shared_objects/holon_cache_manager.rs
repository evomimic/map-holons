use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

use super::{holon_cache::HolonCache, Holon};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::{HolonReference, HolonServiceApi, ReadableHolon};
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

    fn related_holons_with_policy(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        let cache = self
            .relationship_cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {e}"))
            })?
            .clone();
        cache.related_holons(
            context,
            self.holon_service.as_ref(),
            source_holon_id,
            relationship_name,
            || {
                self.holon_service.relationship_cache_policy(
                    context,
                    source_holon_id,
                    relationship_name,
                )
            },
        )
    }
}

impl HolonCacheAccess for HolonCacheManager {
    fn get_cached_rc_holon(
        &self,
        holon_id: &HolonId,
    ) -> Result<Option<Arc<RwLock<Holon>>>, HolonError> {
        Ok(self.cache.get(holon_id))
    }

    /// Retrieves a Holon by its `HolonId`.
    /// - If the Holon is already in the cache, it returns the cached reference.
    /// - Otherwise, it fetches the Holon using the HolonService, adds it to the cache, and returns it.
    /// The behavior of this method is different on the client-side, where all Holons are cached
    /// in a single cache, and the guest-side where each space has its own cache.
    fn get_rc_holon(
        &self,
        context: &Arc<TransactionContext>,
        holon_id: &HolonId,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Attempt to retrieve the holon from the cache
        if let Some(cached) = self.cache.get(holon_id) {
            info!("[PERF-674] holon cache hit");
            debug!("Holon {:?} retrieved from cache.", holon_id);
            return Ok(cached.clone());
        }
        // If not found, resolve it from the HolonService
        info!("[PERF-674] holon cache miss");
        debug!("Holon with HolonId {:?} not in cache. Fetching using HolonService.", holon_id);
        let holon = self.holon_service.fetch_holon_internal(context, holon_id)?;
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
        self.related_holons_with_policy(context, source_holon_id, relationship_name)
    }

    fn get_related_holons_with_hint(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
        hint: crate::RelationshipReadHint,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        let cache = self
            .relationship_cache
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .clone();
        cache.related_holons_with_hint(
            context,
            self.holon_service.as_ref(),
            source,
            name,
            hint,
            || self.holon_service.relationship_cache_policy(context, source, name),
        )
    }

    fn get_all_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let source =
            HolonReference::smart_from_id(context.space_read_handle(), source_holon_id.clone());
        let relationship_names = {
            let mut seen = HashSet::new();
            let mut names = Vec::new();
            for relationship in source.available_relationships()? {
                let name = relationship.descriptor.base_relationship_name()?;
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
            names
        };

        let mut relationship_map = RelationshipMap::new_empty();
        for name in relationship_names {
            let collection = self.get_related_holons(context, source_holon_id, &name)?;
            relationship_map.insert(name, collection);
        }

        Ok(relationship_map)
    }
}

#[cfg(test)]
mod tests;
