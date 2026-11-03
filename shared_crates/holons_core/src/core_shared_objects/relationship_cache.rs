use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use tracing::debug;

use super::holon_cache_manager::CacheReadTransactions;
use super::relationship_fetch::{FetchedRelationship, RelationshipPolicyBinding};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{CollectionState, HolonCollection};
use crate::reference_layer::HolonServiceApi;
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, HolonId, RelationshipName};
/// Selects whether a relationship read may reuse a cached collection for the
/// lifetime of its enclosing cache manager.
///
/// The service resolves eligibility once per cached relationship. Bounded reuse
/// requires a service-provided monotonic clock; request-local guests reuse all
/// membership without descriptor lookup or a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Resolved schema policy survives membership expiration and explicit fresh reads.
/// Only authoritative resolutions create entries; temporary fallbacks remain retryable.
#[derive(Clone, Debug)]
struct CachedRelationship {
    generation: u64,
    policy: RelationshipCachePolicy,
    descriptor: Option<RelationshipPolicyBinding>,
    membership: Option<CachedMembership>,
}

impl CachedRelationship {
    fn new(
        policy: RelationshipCachePolicy,
        collection: Arc<RwLock<HolonCollection>>,
        fetched_at: Option<u64>,
        generation: u64,
        descriptor: Option<RelationshipPolicyBinding>,
    ) -> Self {
        let expiry = match policy {
            RelationshipCachePolicy::Reuse => Some(None),
            RelationshipCachePolicy::MaxAgeMillis(age) if age > 0 => {
                fetched_at.and_then(|start| start.checked_add(age)).map(Some)
            }
            _ => None,
        };
        Self {
            generation,
            policy,
            descriptor,
            membership: expiry
                .map(|expires_at_millis| CachedMembership { collection, expires_at_millis }),
        }
    }
}

impl CachedMembership {
    fn is_unexpired(&self, now: Option<u64>) -> bool {
        self.expires_at_millis.map_or(true, |expiry| now.is_some_and(|now| now < expiry))
    }
}

/// In-memory cache of relationship collections.
///
/// Keys are logically `(source HolonId, relationship name)`. The nested map is
/// only a storage optimization. Entries retain descriptor associations and scalar policies independently
/// of optional saved memberships and their service-selected expiry. Policy lifetime
/// follows the enclosing cache/schema snapshot, not membership freshness.
/// The cache enforces service-selected eligibility, expiry, and caller hints.
#[derive(Clone, Debug)]
pub struct RelationshipCache {
    cache: Arc<RwLock<HashMap<HolonId, HashMap<RelationshipName, CachedRelationship>>>>,
}

// TODO: Consider replacing `HashMap<HolonId, HashMap<RelationshipName, CachedRelationship>>` with a fine-grained
// cache keyed by (HolonId, RelationshipName), e.g. `Cache<(HolonId, RelationshipName), Arc<RwLock<HolonCollection>>>`.
// This would enable lock-free reads and concurrent inserts at the relationship level,
// eliminating the need for outer RwLock and improving cache concurrency.
impl RelationshipCache {
    /// Creates a new RelationshipCache with an empty cache.
    pub fn new() -> Self {
        Self { cache: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Prepares requested saved memberships through the existing retention policy.
    /// A failed/incomplete provider response installs none of its new collections.
    pub(super) fn prepare(
        &self,
        transactions: &CacheReadTransactions,
        service: &dyn HolonServiceApi,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<(), HolonError> {
        let now = service.relationship_cache_time_millis();
        let generation = service.relationship_cache_generation();
        let mut missing = Vec::new();
        let mut policies = HashMap::new();
        let mut descriptors = HashMap::new();
        {
            let cache =
                self.cache.read().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
            for name in names {
                let entry = cache
                    .get(source)
                    .and_then(|entries| entries.get(name))
                    .filter(|entry| entry.generation == generation);
                if let Some(entry) = entry {
                    policies.insert(name.clone(), entry.policy);
                    if let Some(descriptor) = &entry.descriptor {
                        descriptors.insert(name.clone(), descriptor.clone());
                    }
                }
                let resident = entry
                    .and_then(|entry| entry.membership.as_ref())
                    .is_some_and(|membership| membership.is_unexpired(now));
                if !resident && !missing.contains(name) {
                    missing.push(name.clone());
                }
            }
        }
        if missing.is_empty() {
            return Ok(());
        }
        let context = transactions.open()?;
        for name in &missing {
            if !policies.contains_key(name) {
                service.prepare_relationship_policy_internal(&context, source, name)?;
            }
        }
        // Preparation may publish a new schema generation.
        let generation = service.relationship_cache_generation();
        let fetched_at = service.relationship_cache_time_millis();
        let unresolved: Vec<_> =
            missing.iter().filter(|name| !policies.contains_key(*name)).cloned().collect();
        let mut fetched = service.fetch_relationship_set_with_descriptors_internal(
            &context,
            source,
            &missing,
            &unresolved,
        )?;
        if fetched.len() != missing.len()
            || missing
                .iter()
                .any(|name| fetched.iter().filter(|(actual, _)| actual == name).count() != 1)
        {
            return Err(HolonError::InvalidState("Incomplete requested relationship set".into()));
        }
        let mut prepared = Vec::new();
        for (name, fetched) in fetched.drain(..) {
            let descriptor = fetched.descriptor.or_else(|| descriptors.remove(&name));
            let collection = Arc::new(RwLock::new(seal_saved_collection(fetched.collection)?));
            let policy = match policies.get(&name) {
                Some(policy) => Some(*policy),
                None => match &descriptor {
                    Some(binding) => Some(binding.policy),
                    None => service.relationship_cache_policy(&context, source, &name)?,
                },
            };
            if let Some(policy) = policy {
                prepared.push((
                    name,
                    CachedRelationship::new(policy, collection, fetched_at, generation, descriptor),
                ));
            }
        }
        let mut cache =
            self.cache.write().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        cache.entry(source.clone()).or_default().extend(prepared);
        Ok(())
    }

    /// Retrieves the `HolonCollection` containing references to all holons that are related
    /// to the specified `source_holon_id` via the specified `relationship_name` Note
    /// that the `HolonCollection` could be empty.
    ///
    /// Classified cache hits, including known-empty collections, require no classification.
    /// On a membership miss, fetch and seal; consume the fetched descriptor policy
    /// only if none is retained. Policy lookup itself never discovers schema.
    /// No cache lock is held during retrieval.
    pub(super) fn related_holons(
        &self,
        transactions: &CacheReadTransactions,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
        cache_policy: impl FnOnce(
            &Arc<TransactionContext>,
        ) -> Result<Option<RelationshipCachePolicy>, HolonError>,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.related_holons_with_hint(
            transactions,
            holon_service,
            source_holon_id,
            relationship_name,
            RelationshipReadHint::SchemaDefault,
            cache_policy,
        )
    }

    /// Applies caller freshness before returning a cached membership collection.
    pub(super) fn related_holons_with_hint(
        &self,
        transactions: &CacheReadTransactions,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
        hint: RelationshipReadHint,
        cache_policy: impl FnOnce(
            &Arc<TransactionContext>,
        ) -> Result<Option<RelationshipCachePolicy>, HolonError>,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        #[cfg(not(target_arch = "wasm32"))]
        let _profile = tracing::debug_span!(
            target: "map_profile", "relationship_read",
            relationship = %relationship_name
        )
        .entered();
        // Release the cache lock before retrieval. Descriptor binding belongs
        // to that provider operation; policy consumption below is a local read.
        let generation = holon_service.relationship_cache_generation();
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
            .filter(|entry| entry.generation == generation)
            .cloned();
        let retained_policy = cached.as_ref().map(|entry| entry.policy);
        let mut miss_reason = "missing";
        if let Some(membership) = cached.as_ref().and_then(|entry| entry.membership.as_ref()) {
            if hint == RelationshipReadHint::SchemaDefault {
                if membership.is_unexpired(holon_service.relationship_cache_time_millis()) {
                    super::read_observations::record(
                        source_holon_id,
                        Some(relationship_name),
                        "hit",
                    );
                    #[cfg(not(target_arch = "wasm32"))]
                    tracing::debug!(target: "map_profile", cache = "hit");
                    return Ok(Arc::clone(&membership.collection));
                }
                miss_reason = "expired";
            } else {
                miss_reason = "fresh";
            }
        }
        super::read_observations::record(source_holon_id, Some(relationship_name), miss_reason);

        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(target: "map_profile", cache = "miss");
        // Cache miss: Fetch related holons from the HolonServiceApi
        debug!(
        "Cache miss for source_holon_id: {:?}, relationship_name: {:?}. Fetching from HolonServiceApi.",
        source_holon_id, relationship_name
    );
        let context = transactions.open()?;
        if retained_policy.is_none() {
            holon_service.prepare_relationship_policy_internal(
                &context,
                source_holon_id,
                relationship_name,
            )?;
        }
        let generation = holon_service.relationship_cache_generation();
        // Start TTL before the fetch, so transport time never extends freshness.
        let fetched_at = holon_service.relationship_cache_time_millis();
        let fetched = if retained_policy.is_some() {
            FetchedRelationship {
                collection: holon_service.fetch_related_holons_internal(
                    &context,
                    source_holon_id,
                    relationship_name,
                )?,
                descriptor: cached.as_ref().and_then(|entry| entry.descriptor.clone()),
            }
        } else {
            holon_service.fetch_related_holons_with_descriptor_internal(
                &context,
                source_holon_id,
                relationship_name,
            )?
        };
        let descriptor = fetched.descriptor;
        let fetched_arc = {
            #[cfg(not(target_arch = "wasm32"))]
            let _seal = tracing::debug_span!(target: "map_profile", "seal_collection").entered();
            Arc::new(RwLock::new(seal_saved_collection(fetched.collection)?))
        };

        let policy = match retained_policy {
            Some(policy) => Some(policy),
            None => {
                #[cfg(not(target_arch = "wasm32"))]
                let _policy =
                    tracing::debug_span!(target: "map_profile", "relationship_policy").entered();
                match &descriptor {
                    Some(binding) => Some(binding.policy),
                    None => cache_policy(&context)?,
                }
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(target: "map_profile", ?policy);
        // A temporary unresolved fallback serves this read fresh, but must not
        // become a permanent Fresh policy for later, fully resolvable reads.
        if let Some(policy) = policy {
            self.cache
                .write()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Relationship cache write lock poisoned: {e}"
                    ))
                })?
                .entry(source_holon_id.clone())
                .or_default()
                .insert(
                    relationship_name.clone(),
                    CachedRelationship::new(
                        policy,
                        Arc::clone(&fetched_arc),
                        fetched_at,
                        generation,
                        descriptor,
                    ),
                );
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
