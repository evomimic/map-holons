use std::collections::HashSet;
use std::sync::{Arc, RwLock, Weak};
use tracing::{debug, info};

use super::{holon_cache::HolonCache, Holon};
use crate::core_shared_objects::transactions::PrivateReadTransaction;
#[cfg(test)]
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::Descriptor;
use crate::reference_layer::{HolonReference, HolonServiceApi};
use crate::{HolonCacheAccess, HolonCollection, ReadableHolon, RelationshipCache, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Debug, Clone)]
pub struct HolonCacheManager {
    cache: HolonCache, // Thread-safe cache of holons
    relationship_cache: Arc<RwLock<RelationshipCache>>,
    holon_service: Arc<dyn HolonServiceApi>,
    /// Opens isolated execution contexts only when saved-state retrieval needs them.
    transactions: CacheReadTransactions,
}

/// Space-bound capability for opening private cache-read transactions on demand.
///
/// Saved references belong to a Space and may be reused across public transactions.
/// A cache miss therefore cannot borrow the transaction that originally minted a
/// reference. Host service fetches still require a `TransactionContext` for the
/// normal Dance request/response transport, and relationship-policy preparation
/// may also require execution through that service. Cache internals use this
/// capability only after determining that preparation or retrieval is required;
/// eligible cache hits need no transaction.
///
/// Each opened context has isolated temporary pools, prohibits staging and Commit,
/// and is absent from the public transaction registry. The returned guard owns its
/// cleanup. This capability retains no contexts and holds its Space weakly because
/// the Space already owns the cache manager; a strong back-reference would form an
/// ownership cycle. It fails if the owning Space is no longer available.
#[derive(Debug, Clone)]
pub(super) struct CacheReadTransactions {
    space: Weak<super::space_manager::HolonSpaceManager>,
}

impl CacheReadTransactions {
    /// Acquires the owning Space for the duration of an internal cache operation.
    fn space(&self) -> Result<Arc<super::space_manager::HolonSpaceManager>, HolonError> {
        self.space
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("Cache owning Space".into()))
    }

    /// Opens an unregistered restricted context with cleanup bound to the returned guard.
    pub(super) fn open(&self) -> Result<PrivateReadTransaction, HolonError> {
        let space = self.space()?;
        space.get_transaction_manager().open_private_saved_read(space)
    }
}

impl HolonCacheManager {
    /// Binds saved caches to their service and a weak owning-Space transaction capability.
    ///
    /// `space` must identify the Space owning this cache and its service. Construction
    /// stores the weak binding without opening a transaction or upgrading it, allowing
    /// the Space to provision its cache during `Arc::new_cyclic` initialization.
    pub fn new(
        holon_service: Arc<dyn HolonServiceApi>,
        space: Weak<super::space_manager::HolonSpaceManager>,
    ) -> Self {
        Self {
            cache: HolonCache::new(),
            relationship_cache: Arc::new(RwLock::new(RelationshipCache::new())),
            holon_service,
            transactions: CacheReadTransactions { space },
        }
    }

    fn related_holons_with_policy(
        &self,
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
            &self.transactions,
            self.holon_service.as_ref(),
            source_holon_id,
            relationship_name,
            |context| {
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
    fn ensure_cached(&self, ids: &[HolonId]) -> Vec<super::cache_population::CachePopulationError> {
        use super::cache_population::{validate_saved_content, CachePopulationError};
        use std::collections::HashMap;
        let mut seen = HashSet::new();
        let distinct: Vec<_> =
            ids.iter().filter(|id| seen.insert((*id).clone())).cloned().collect();
        let missing: Vec<_> =
            distinct.iter().filter(|id| self.cache.get(id).is_none()).cloned().collect();
        let mut errors = Vec::new();
        let mut populated = 0;
        if !missing.is_empty() {
            let fetched = self
                .transactions
                .open()
                .and_then(|context| self.holon_service.fetch_holons_internal(&context, &missing));
            match fetched {
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

    /// Retrieves a Holon by its `HolonId`.
    /// - If the Holon is already in the cache, it returns the cached reference.
    /// - Otherwise, it fetches the Holon using the HolonService, adds it to the cache, and returns it.
    /// The behavior of this method is different on the client-side, where all Holons are cached
    /// in a single cache, and the guest-side where each space has its own cache.
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Arc<RwLock<Holon>>, HolonError> {
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
        let holon = {
            let context = self.transactions.open()?;
            self.holon_service.fetch_holon_internal(&context, holon_id)?
        };
        super::cache_population::validate_saved_content(holon_id, &holon)?;
        let arc_holon = Arc::new(RwLock::new(holon));

        // Insert the resolved holon into the cache
        let arc_holon = self.cache.insert_if_absent(holon_id.clone(), arc_holon)?;
        debug!("Holon with HolonId {:?} fetched and cached.", holon_id);

        Ok(arc_holon)
    }

    fn get_related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.related_holons_with_policy(source_holon_id, relationship_name)
    }

    fn get_related_holons_with_hint(
        &self,
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
            &self.transactions,
            self.holon_service.as_ref(),
            source,
            name,
            hint,
            |context| self.holon_service.relationship_cache_policy(context, source, name),
        )
    }

    fn retain_holon(
        &self,
        id: &HolonId,
    ) -> Result<super::holon_cache::SavedHolonRetention, HolonError> {
        self.cache.retain(id.clone(), self.get_rc_holon(id)?)
    }

    fn prepare_relationships(
        &self,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<(), HolonError> {
        let cache = self
            .relationship_cache
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .clone();
        cache.prepare(&self.transactions, self.holon_service.as_ref(), source, names)
    }

    fn get_all_related_holons(
        &self,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let source = HolonReference::smart_from_id(
            crate::core_shared_objects::SpaceReadHandle::new(self.transactions.space()?),
            source_holon_id.clone(),
        );
        let relationship_names = {
            let mut seen = HashSet::new();
            let mut names = Vec::new();
            // Complete content reads and saved clones must retain the authored
            // contract even when presentation navigation omits a declaration.
            let descriptor = source.holon_descriptor()?;
            let declared = match crate::descriptors::EffectiveHolonDescriptor::find_prepared(
                descriptor.holon(),
            )? {
                Some(prepared) => prepared.instance_relationships()?,
                None => crate::descriptors::effective_relationships::effective_declared_relationships_for_holon(&source)?,
            };
            for relationship in declared {
                let name = relationship.base_relationship_name()?;
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
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
            let collection = self.get_related_holons(source_holon_id, &name)?;
            relationship_map.insert(name, collection);
        }

        Ok(relationship_map)
    }
}

#[cfg(test)]
mod tests;
