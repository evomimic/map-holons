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
        cache_policy: RelationshipCachePolicy,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.relationship_cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!("Cache manager read lock poisoned: {e}"))
            })?
            .related_holons(
                context,
                self.holon_service.as_ref(),
                source_holon_id,
                relationship_name,
                cache_policy,
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
        match self.holon_service.relationship_cache_scope() {
            // A guest cache manager is created for one dance request, so it
            // may reuse non-definitional and inverse membership within that
            // request without asserting cross-transaction immutability.
            RelationshipCacheScope::RequestLocal => Ok(RelationshipCachePolicy::Reuse),
            RelationshipCacheScope::SpaceDefinitionalOnly => {
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
        let cache_policy =
            self.relationship_cache_policy(context, source_holon_id, relationship_name)?;
        self.related_holons_with_policy(context, source_holon_id, relationship_name, cache_policy)
    }

    fn get_all_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        if self.holon_service.relationship_cache_scope() == RelationshipCacheScope::RequestLocal {
            return self.holon_service.fetch_all_related_holons_internal(context, source_holon_id);
        }

        if self.resolving_relationship_semantics() {
            return self.holon_service.fetch_all_related_holons_internal(context, source_holon_id);
        }
        let _resolution_guard = self.enter_relationship_semantics_resolution();

        let source =
            HolonReference::smart_from_id(context.space_read_handle(), source_holon_id.clone());
        let declared_relationships = match source.holon_descriptor() {
            Ok(descriptor) => descriptor.effective_declared_relationships()?,
            Err(HolonError::MissingDescribedBy { .. }) => {
                return self
                    .holon_service
                    .fetch_all_related_holons_internal(context, source_holon_id)
            }
            Err(error) => return Err(error),
        };

        let mut relationship_map = RelationshipMap::new_empty();
        for declared in declared_relationships {
            let relationship_name = declared.base_relationship_name()?;
            let cache_policy = if declared.is_definitional()? {
                RelationshipCachePolicy::Reuse
            } else {
                RelationshipCachePolicy::Fresh
            };
            let collection = self.related_holons_with_policy(
                context,
                source_holon_id,
                &relationship_name,
                cache_policy,
            )?;
            relationship_map.insert(relationship_name, collection);
        }

        // Inverse relationship membership and any persisted relationship not
        // licensed as a declared source relationship remain fresh. The raw
        // response also avoids deriving inverse navigation from the target-side
        // index merely to answer an all-related read for this source.
        let fresh_relationships =
            self.holon_service.fetch_all_related_holons_internal(context, source_holon_id)?;
        for (relationship_name, collection) in fresh_relationships.iter() {
            if relationship_map.get_collection_for_relationship(&relationship_name).is_none() {
                relationship_map.insert(relationship_name, collection);
            }
        }

        Ok(relationship_map)
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
