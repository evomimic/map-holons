use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

use super::{holon_cache::HolonCache, Holon};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::{HolonReference, HolonServiceApi};
use crate::{HolonCacheAccess, HolonCollection, RelationshipCache, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Debug, Clone)]
pub struct HolonCacheManager {
    cache: HolonCache, // Thread-safe cache of holons
    relationship_cache: Arc<RwLock<RelationshipCache>>,
    holon_service: Arc<dyn HolonServiceApi>,
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(holon_service: Arc<dyn HolonServiceApi>) -> Self {
        Self {
            cache: HolonCache::new(),
            relationship_cache: Arc::new(RwLock::new(RelationshipCache::new())),
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
    fn ensure_cached(
        &self,
        context: &Arc<TransactionContext>,
        ids: &[HolonId],
    ) -> Vec<super::cache_population::CachePopulationError> {
        use super::cache_population::{validate_saved_content, CachePopulationError};
        use std::collections::HashMap;
        if !ids.is_empty() && !context.is_restricted_cache_read() {
            return vec![CachePopulationError::new(
                ids.to_vec(),
                HolonError::InvalidParameter(
                    "Saved cache population requires a restricted cache-read context".into(),
                ),
            )];
        }
        let mut seen = HashSet::new();
        let distinct: Vec<_> =
            ids.iter().filter(|id| seen.insert((*id).clone())).cloned().collect();
        let missing: Vec<_> =
            distinct.iter().filter(|id| self.cache.get(id).is_none()).cloned().collect();
        let mut errors = Vec::new();
        let mut populated = 0;
        if !missing.is_empty() {
            match self.holon_service.fetch_holons_internal(context, &missing) {
                Err(error) => errors.push(CachePopulationError::new(missing.clone(), error)),
                Ok(entries) => {
                    let requested: HashSet<_> = missing.iter().cloned().collect();
                    let mut grouped: HashMap<_, Vec<_>> = HashMap::new();
                    for entry in entries {
                        if !requested.contains(&entry.id) {
                            errors.push(CachePopulationError::new(
                                vec![entry.id],
                                HolonError::InvalidParameter(
                                    "Unexpected saved hydration identity".into(),
                                ),
                            ));
                        } else {
                            grouped.entry(entry.id).or_default().push(entry.result);
                        }
                    }
                    for id in &missing {
                        let results = grouped.remove(id).unwrap_or_default();
                        let result = if results.len() != 1 {
                            Err(HolonError::InvalidParameter(format!(
                                "Saved hydration expected one response for {id:?}, received {}",
                                results.len()
                            )))
                        } else {
                            results.into_iter().next().unwrap().and_then(|holon| {
                                validate_saved_content(id, &holon)?;
                                self.cache
                                    .insert_if_absent(id.clone(), Arc::new(RwLock::new(holon)))?;
                                Ok(())
                            })
                        };
                        match result {
                            Ok(()) => populated += 1,
                            Err(error) => {
                                errors.push(CachePopulationError::new(vec![id.clone()], error))
                            }
                        }
                    }
                }
            }
        }
        info!(target: "map_profile", requested = ids.len(), distinct = distinct.len(), hits = distinct.len() - missing.len(), missing = missing.len(), populated, errors = errors.len(), "saved cache population");
        errors
    }

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
            super::read_observations::record(holon_id, None, "hit");
            info!("[PERF-674] holon cache hit");
            debug!("Holon {:?} retrieved from cache.", holon_id);
            return Ok(cached.clone());
        }
        super::read_observations::record(holon_id, None, "missing");
        // If not found, resolve it from the HolonService
        info!("[PERF-674] holon cache miss");
        debug!("Holon with HolonId {:?} not in cache. Fetching using HolonService.", holon_id);
        let holon = self.holon_service.fetch_holon_internal(context, holon_id)?;
        super::cache_population::validate_saved_content(holon_id, &holon)?;
        let arc_holon = Arc::new(RwLock::new(holon));

        // Insert the resolved holon into the cache
        let arc_holon = self.cache.insert_if_absent(holon_id.clone(), arc_holon)?;
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

    fn retain_holon(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<super::holon_cache::SavedHolonRetention, HolonError> {
        self.cache.retain(id.clone(), self.get_rc_holon(context, id)?)
    }

    fn prepare_relationships(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<(), HolonError> {
        let cache = self
            .relationship_cache
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .clone();
        cache.prepare(context, self.holon_service.as_ref(), source, names)
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
            for relationship in
                crate::reference_layer::available_relationships::available_relationships(&source)?
            {
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
