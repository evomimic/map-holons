use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use tracing::debug;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{CollectionState, HolonCollection, RelationshipMap};
use crate::reference_layer::HolonServiceApi;
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, HolonId, RelationshipName};
/// Selects whether a relationship read may reuse a cached collection for the
/// lifetime of its enclosing cache manager.
///
/// The host determines whether a saved relationship is stable enough for its
/// space-wide cache. Guest cache managers are request-local and therefore may
/// safely reuse any persisted relationship collection for that request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipCachePolicy {
    Reuse,
    Fresh,
}

/// In-memory cache of relationship collections.
///
/// Keys are logically `(source HolonId, relationship name)`. The nested map is
/// only a storage optimization. Entries contain saved references whose
/// membership is reusable for the cache manager's lifetime. The owning cache
/// manager is responsible for ensuring that a space-wide cache stores only
/// declared definitional relationship collections.
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

    /// Retrieves the `HolonCollection` containing references to all holons that are related
    /// to the specified `source_holon_id` via the specified `relationship_name` Note
    /// that the `HolonCollection` could be empty.
    ///
    /// Reusable reads are cached, including known-empty collections. Fresh
    /// reads are deliberately never read from or written to this cache.
    pub fn related_holons(
        &self,
        context: &Arc<TransactionContext>,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
        cache_policy: RelationshipCachePolicy,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        if cache_policy == RelationshipCachePolicy::Reuse {
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
        }

        // Cache miss: Fetch related holons from the HolonServiceApi
        debug!(
        "Cache miss for source_holon_id: {:?}, relationship_name: {:?}. Fetching from HolonServiceApi.",
        source_holon_id, relationship_name
    );
        let fetched_holons = holon_service.fetch_related_holons_internal(
            context,
            &source_holon_id,
            relationship_name,
        )?;
        let fetched_arc = Arc::new(RwLock::new(seal_saved_collection(fetched_holons)?));

        if cache_policy == RelationshipCachePolicy::Reuse {
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
        Ok(fetched_arc)
    }
}

fn seal_saved_collection(fetched: HolonCollection) -> Result<HolonCollection, HolonError> {
    let members = fetched.get_members().clone();
    let mut keyed_index = BTreeMap::new();
    for (index, reference) in members.iter().enumerate() {
        if !matches!(reference, HolonReference::Smart(_)) {
            return Err(HolonError::InvalidState(
                "relationship cache may retain only saved references".to_owned(),
            ));
        }
        if let Some(key) = reference.key()? {
            keyed_index.insert(key, index);
        }
    }
    Ok(HolonCollection::from_parts(CollectionState::Saved, members, keyed_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_thread_safe<T: Send + Sync>() {}

    #[test]
    fn relationship_cache_is_thread_safe() {
        assert_thread_safe::<RelationshipCache>();
    }
}
