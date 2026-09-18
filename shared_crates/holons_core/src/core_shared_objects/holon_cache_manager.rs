use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, RwLock,
};
use tracing::{debug, info};

use super::{holon_cache::HolonCache, Holon};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::{
    HolonReference, HolonServiceApi, ReadableHolon, RelationshipCacheScope,
};
use crate::{
    HolonCacheAccess, HolonCollection, RelationshipCache, RelationshipCachePolicy, RelationshipMap,
};
use core_types::{HolonError, HolonId, RelationshipName};
use type_names::{CoreRelationshipTypeName, ToRelationshipName};

#[derive(Debug)]
pub struct HolonCacheManager {
    cache: HolonCache, // Thread-safe cache of holons
    relationship_cache: RwLock<RelationshipCache>,
    holon_service: Arc<dyn HolonServiceApi>,
    /// Prevents descriptor traversal used to classify a host cache read from
    /// recursively re-entering that same classification path.
    relationship_semantics_resolution_depth: AtomicUsize,
}

struct RelationshipSemanticsResolutionGuard<'a> {
    depth: &'a AtomicUsize,
}

impl Drop for RelationshipSemanticsResolutionGuard<'_> {
    fn drop(&mut self) {
        self.depth.fetch_sub(1, Ordering::Release);
    }
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(holon_service: Arc<dyn HolonServiceApi>) -> Self {
        Self {
            cache: HolonCache::new(),
            relationship_cache: RwLock::new(RelationshipCache::new()),
            holon_service,
            relationship_semantics_resolution_depth: AtomicUsize::new(0),
        }
    }

    fn resolving_relationship_semantics(&self) -> bool {
        self.relationship_semantics_resolution_depth.load(Ordering::Acquire) > 0
    }

    fn enter_relationship_semantics_resolution(&self) -> RelationshipSemanticsResolutionGuard<'_> {
        self.relationship_semantics_resolution_depth.fetch_add(1, Ordering::AcqRel);
        RelationshipSemanticsResolutionGuard {
            depth: &self.relationship_semantics_resolution_depth,
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
            || self.relationship_cache_policy(context, source_holon_id, relationship_name),
        )
    }

    fn space_relationship_cache_policy(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        // These kernel structural edges define the immutable descriptor graph
        // used to classify every application relationship. Resolving their
        // cache policy through that same graph would recurse indefinitely;
        // their membership is version-bound for every saved source and may be
        // reused without further descriptor traversal.
        if matches!(
            relationship_name,
            name if name == &CoreRelationshipTypeName::DescribedBy.to_relationship_name()
                || name == &CoreRelationshipTypeName::Extends.to_relationship_name()
                || name == &CoreRelationshipTypeName::InstanceRelationships.to_relationship_name()
        ) {
            return Ok(RelationshipCachePolicy::Reuse);
        }

        // Resolving the declared source contract traverses the descriptor graph
        // through ordinary relationship reads. Those reads must not recursively
        // attempt to classify themselves for the space-wide cache. They remain
        // correct as fresh reads; the guard is deliberately conservative across
        // concurrent callers, where it may cause an additional fresh read but
        // never reuse mutable relationship membership.
        if self.resolving_relationship_semantics() {
            return Ok(RelationshipCachePolicy::Fresh);
        }
        let _resolution_guard = self.enter_relationship_semantics_resolution();

        let source =
            HolonReference::smart_from_id(context.space_read_handle(), source_holon_id.clone());
        match source.holon_descriptor() {
            Ok(descriptor) => descriptor
                .effective_declared_relationships()?
                .into_iter()
                .find(|declared| {
                    declared.base_relationship_name().is_ok_and(|name| name == *relationship_name)
                })
                .map(|declared| {
                    if declared.is_definitional()? {
                        Ok(RelationshipCachePolicy::Reuse)
                    } else {
                        Ok(RelationshipCachePolicy::Fresh)
                    }
                })
                // An inverse or unknown name is not part of the immutable
                // source contract. Preserve read behavior, but never retain
                // its membership in the cross-transaction cache.
                .unwrap_or(Ok(RelationshipCachePolicy::Fresh)),
            // Legacy saved holons may predate the `DescribedBy` contract. They
            // cannot establish cross-transaction stability, but remain readable.
            Err(HolonError::MissingDescribedBy { .. }) => Ok(RelationshipCachePolicy::Fresh),
            Err(error) => Err(error),
        }
    }

    fn relationship_cache_policy(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        // Cache lifetime does not make mutable membership immutable. Both
        // request-local and space caches use the same descriptor-governed policy.
        match self.holon_service.relationship_cache_scope() {
            RelationshipCacheScope::RequestLocal
            | RelationshipCacheScope::SpaceDefinitionalOnly => {
                self.space_relationship_cache_policy(context, source_holon_id, relationship_name)
            }
        }
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

    fn get_all_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let source =
            HolonReference::smart_from_id(context.space_read_handle(), source_holon_id.clone());
        let relationship_names = {
            // Discovery reads the descriptor graph through this cache manager.
            // Release its recursion guard before named reads classify membership
            // for reuse, otherwise eligible collections would be forced fresh.
            let _resolution_guard = self.enter_relationship_semantics_resolution();
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
