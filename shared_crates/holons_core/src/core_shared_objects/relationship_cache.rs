use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use tracing::debug;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{CollectionState, HolonCollection};
use crate::reference_layer::HolonServiceApi;
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, HolonId, RelationshipName};
/// Selects whether a relationship read may reuse a cached collection for the
/// lifetime of its enclosing cache manager.
///
/// The service decides eligibility before a retained collection is reused. Bounded reuse
/// requires a service-provided monotonic clock; request-local guests reuse all
/// membership without descriptor lookup or a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipCachePolicy {
    /// Reuse for this cache lifetime, as selected by the owning service.
    Reuse,
    /// Mutable membership may be reused for at most this many milliseconds.
    MaxAgeMillis(u64),
    /// Fetch membership on every read.
    Fresh,
}

/// Caller freshness requirement. A fresh read bypasses membership caching;
/// it does not promise a globally synchronized DHT snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RelationshipReadHint {
    #[default]
    SchemaDefault,
    RequireFresh,
}

#[derive(Clone, Debug)]
struct CachedMembership {
    collection: Arc<RwLock<HolonCollection>>,
    expires_at_millis: Option<u64>,
}

/// In-memory cache of relationship collections.
///
/// Keys are logically `(source HolonId, relationship name)`. The nested map is
/// only a storage optimization. Entries contain saved references whose
/// membership is immutable or reusable until its service-selected expiry.
/// The cache enforces service-selected eligibility, expiry, and caller hints.
#[derive(Clone, Debug)]
pub struct RelationshipCache {
    cache: Arc<RwLock<HashMap<HolonId, HashMap<RelationshipName, CachedMembership>>>>,
}

// TODO: Consider replacing `HashMap<HolonId, HashMap<RelationshipName, CachedMembership>>` with a fine-grained
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
    /// Classified cache hits, including known-empty collections, require no classification.
    /// On a miss, fetch and seal before asking the service about retention.
    /// No cache lock is held during policy resolution.
    pub fn related_holons(
        &self,
        context: &Arc<TransactionContext>,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
        cache_policy: impl FnOnce() -> Result<RelationshipCachePolicy, HolonError>,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.related_holons_with_hint(
            context,
            holon_service,
            source_holon_id,
            relationship_name,
            RelationshipReadHint::SchemaDefault,
            cache_policy,
        )
    }

    /// Applies caller freshness before returning a cached membership collection.
    pub fn related_holons_with_hint(
        &self,
        context: &Arc<TransactionContext>,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
        hint: RelationshipReadHint,
        cache_policy: impl FnOnce() -> Result<RelationshipCachePolicy, HolonError>,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        #[cfg(not(target_arch = "wasm32"))]
        let _profile = tracing::debug_span!(
            target: "map_profile", "relationship_read",
            relationship = %relationship_name,
            tx_id = context.tx_id().value()
        )
        .entered();
        // Release the cache lock before classification: descriptor traversal
        // resolves its own relationship reads through this same cache.
        let cached = self
            .cache
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "relationship cache read lock poisoned: {e}"
                ))
            })?
            .get(source_holon_id)
            .and_then(|relationships| relationships.get(relationship_name))
            .cloned();
        if let Some(cached) = cached {
            if hint == RelationshipReadHint::SchemaDefault {
                let unexpired = match cached.expires_at_millis {
                    None => true,
                    Some(deadline) => holon_service
                        .relationship_cache_time_millis()
                        .is_some_and(|now| now < deadline),
                };
                if unexpired {
                    #[cfg(not(target_arch = "wasm32"))]
                    tracing::debug!(target: "map_profile", cache = "hit");
                    debug!(
                        "Cache hit for source_holon_id: {:?}, relationship_name: {:?}",
                        source_holon_id, relationship_name
                    );
                    return Ok(cached.collection);
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(target: "map_profile", cache = "miss");
        // Cache miss: Fetch related holons from the HolonServiceApi
        debug!(
        "Cache miss for source_holon_id: {:?}, relationship_name: {:?}. Fetching from HolonServiceApi.",
        source_holon_id, relationship_name
    );
        // Start TTL before the fetch, so transport time never extends freshness.
        let fetched_at = holon_service.relationship_cache_time_millis();
        let fetched_holons = holon_service.fetch_related_holons_internal(
            context,
            &source_holon_id,
            relationship_name,
        )?;
        let fetched_arc = {
            #[cfg(not(target_arch = "wasm32"))]
            let _seal = tracing::debug_span!(target: "map_profile", "seal_collection").entered();
            Arc::new(RwLock::new(seal_saved_collection(fetched_holons)?))
        };

        let policy = {
            #[cfg(not(target_arch = "wasm32"))]
            let _policy =
                tracing::debug_span!(target: "map_profile", "relationship_policy").entered();
            cache_policy()?
        };
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(target: "map_profile", ?policy);
        let retention = match policy {
            RelationshipCachePolicy::Reuse => Some(None),
            RelationshipCachePolicy::MaxAgeMillis(age) if age > 0 => {
                fetched_at.and_then(|start| start.checked_add(age)).map(Some)
            }
            _ => None,
        };
        let mut cache = self.cache.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Relationship cache write lock poisoned: {e}"))
        })?;
        if let Some(expires_at_millis) = retention {
            cache.entry(source_holon_id.clone()).or_default().insert(
                relationship_name.clone(),
                CachedMembership { collection: Arc::clone(&fetched_arc), expires_at_millis },
            );
        } else if let Some(relationships) = cache.get_mut(source_holon_id) {
            relationships.remove(relationship_name);
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
