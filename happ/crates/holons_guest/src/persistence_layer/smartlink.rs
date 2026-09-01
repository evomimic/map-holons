//! Storage-layer SmartLink persistence API (Storage SL1 Part 2, Issue #594; occurrence
//! semantics activated by Storage SL3).
//!
//! Descriptor-unaware read/write/delete over `LinkTypes::SmartLink`. Callers supply
//! fully-prepared inputs; this layer computes no canonical keys and selects no
//! target-property cache candidates. Descriptor-unawareness here is structural, not a
//! convention: no descriptor type is reachable from this module, so whether a relationship
//! may carry duplicates is necessarily decided upstream — see `DuplicatePolicy` in
//! `holons_core::reference_layer`, which is where `AllowsDuplicates` is actually enforced.
//!
//! A supplied `occurrence_id` is honored as part of insertion identity and round-tripped
//! verbatim. Generating one, defaulting it, and pairing it across the declared and inverse
//! directions all stay above storage.
//! It reuses the Tag v1 codec (`core_types::smartlink`) for all encode/decode/prefix work.

use core_types::{
    decode_smartlink, encode_smartlink_tag, smartlink_exact_key_prefix, smartlink_key_prefix,
    smartlink_relationship_prefix, DeleteSmartLinkOutcome, HolonError, KeyMatch, PreparedSmartLink,
    PutSmartLinkOutcome, SmartLink, SmartLinkId,
};
use hdk::prelude::*;
use holons_guest_integrity::type_conversions::*;
use holons_integrity::LinkTypes;
use integrity_core_types::{LocalId, RelationshipName};
use shared_validation::validate_smartlink_envelope;
use std::collections::HashMap;

/// Commit-local cache for SmartLink identity checks.
///
/// This is intentionally constructed by relationship commit orchestration rather than stored on a
/// transaction. A bucket is populated from one complete, fail-closed storage expansion and is
/// discarded when that commit returns.
#[derive(Default)]
pub(crate) struct SmartLinkWriteContext {
    buckets: HashMap<SmartLinkBucketKey, SmartLinkBucket>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SmartLinkBucketKey {
    source_id: LocalId,
    relationship_name: RelationshipName,
}

impl SmartLinkBucketKey {
    fn from_prepared(prepared: &PreparedSmartLink) -> Self {
        Self {
            source_id: prepared.source_id.clone(),
            relationship_name: prepared.relationship_name.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SmartLinkIdentity {
    source_id: LocalId,
    target_id: core_types::HolonId,
    relationship_name: RelationshipName,
    occurrence_id: Option<core_types::OccurrenceId>,
}

impl SmartLinkIdentity {
    fn from_prepared(prepared: &PreparedSmartLink) -> Self {
        Self {
            source_id: prepared.source_id.clone(),
            target_id: prepared.target_id.clone(),
            relationship_name: prepared.relationship_name.clone(),
            occurrence_id: prepared.occurrence_id,
        }
    }

    fn from_smartlink(smartlink: &SmartLink) -> Self {
        Self {
            source_id: smartlink.source_id.clone(),
            target_id: smartlink.target_id.clone(),
            relationship_name: smartlink.relationship_name.clone(),
            occurrence_id: smartlink.occurrence_id,
        }
    }
}

/// The fields relevant to identity conflict decisions. Optional target-property cache values are
/// deliberately omitted because they do not participate in idempotency.
struct CachedSmartLink {
    smartlink_id: SmartLinkId,
    canonical_key: core_types::CanonicalKey,
    relationship_property_values: integrity_core_types::PropertyMap,
}

impl CachedSmartLink {
    fn from_smartlink(smartlink: SmartLink) -> Self {
        Self {
            smartlink_id: smartlink.smartlink_id,
            canonical_key: smartlink.canonical_key,
            relationship_property_values: smartlink.relationship_property_values,
        }
    }

    fn from_inserted(prepared: &PreparedSmartLink, smartlink_id: SmartLinkId) -> Self {
        Self {
            smartlink_id,
            canonical_key: prepared.canonical_key.clone(),
            relationship_property_values: prepared.relationship_property_values.clone(),
        }
    }

    fn outcome_for(&self, prepared: &PreparedSmartLink) -> PutSmartLinkOutcome {
        if self.canonical_key == prepared.canonical_key
            && self.relationship_property_values == prepared.relationship_property_values
        {
            PutSmartLinkOutcome::AlreadyPresent(self.smartlink_id.clone())
        } else {
            PutSmartLinkOutcome::Conflict(self.smartlink_id.clone())
        }
    }
}

#[derive(Default)]
struct SmartLinkBucket {
    identities: HashMap<SmartLinkIdentity, CachedSmartLink>,
}

impl SmartLinkBucket {
    fn from_expansion(smartlinks: Vec<SmartLink>) -> Self {
        let mut bucket = Self::default();
        for smartlink in smartlinks {
            bucket.insert(
                SmartLinkIdentity::from_smartlink(&smartlink),
                CachedSmartLink::from_smartlink(smartlink),
            );
        }
        bucket
    }

    fn insert(&mut self, identity: SmartLinkIdentity, candidate: CachedSmartLink) {
        match self.identities.get(&identity) {
            Some(existing) if existing.smartlink_id.0 .0 <= candidate.smartlink_id.0 .0 => {}
            _ => {
                self.identities.insert(identity, candidate);
            }
        }
    }
}

impl SmartLinkWriteContext {
    fn bucket_for<F>(
        &mut self,
        key: SmartLinkBucketKey,
        expand: F,
    ) -> Result<&mut SmartLinkBucket, HolonError>
    where
        F: FnOnce() -> Result<Vec<SmartLink>, HolonError>,
    {
        if !self.buckets.contains_key(&key) {
            self.buckets.insert(key.clone(), SmartLinkBucket::from_expansion(expand()?));
        }
        Ok(self.buckets.get_mut(&key).expect("inserted or previously cached bucket"))
    }
}

// ---------------------------------------------------------------------------
// Read: expansion
// ---------------------------------------------------------------------------

/// Returns every live SmartLink from `source_id`, across all relationships.
///
/// Results are unordered, unhydrated, and each preserves its physical `SmartLinkId`.
/// If any matching live link is malformed, the whole expansion fails with
/// `HolonError::InvalidWireFormat` naming the offending `SmartLinkId`.
pub fn expand_all_from_source(source_id: &LocalId) -> Result<Vec<SmartLink>, HolonError> {
    let base = try_action_hash_from_local_id(source_id)?;
    let query =
        LinkQuery::try_new(base, LinkTypes::SmartLink).map_err(holon_error_from_wasm_error)?;
    decode_query(source_id, query)
}

/// Returns live SmartLinks from `source_id` filtered to a single relationship.
pub fn expand_from_source(
    source_id: &LocalId,
    relationship_name: &RelationshipName,
) -> Result<Vec<SmartLink>, HolonError> {
    let prefix = LinkTag(smartlink_relationship_prefix(relationship_name).map_err(tag_error)?);
    let base = try_action_hash_from_local_id(source_id)?;
    let query = LinkQuery::try_new(base, LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?
        .tag_prefix(prefix);
    decode_query(source_id, query)
}

/// Returns live SmartLinks from `source_id` for a relationship, filtered by canonical
/// key: exact match, or a non-empty starts-with prefix.
///
/// `StartsWith("")` is rejected with `HolonError::InvalidParameter` — an empty prefix
/// would match every key and is never a meaningful key query.
pub fn expand_from_source_by_key(
    source_id: &LocalId,
    relationship_name: &RelationshipName,
    key_match: KeyMatch,
) -> Result<Vec<SmartLink>, HolonError> {
    let prefix_bytes = match &key_match {
        KeyMatch::Exact(key) => {
            smartlink_exact_key_prefix(relationship_name, key).map_err(tag_error)?
        }
        KeyMatch::StartsWith(prefix) => {
            if prefix.as_str().is_empty() {
                return Err(HolonError::InvalidParameter(
                    "expand_from_source_by_key: StartsWith(\"\") is not a valid key prefix"
                        .to_string(),
                ));
            }
            smartlink_key_prefix(relationship_name, prefix).map_err(tag_error)?
        }
    };
    let base = try_action_hash_from_local_id(source_id)?;
    let query = LinkQuery::try_new(base, LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?
        .tag_prefix(LinkTag(prefix_bytes));
    decode_query(source_id, query)
}

// ---------------------------------------------------------------------------
// Write: idempotent insert with conflict detection
// ---------------------------------------------------------------------------

/// Idempotently persists `prepared`, detecting semantic conflicts.
///
/// Intrinsically invalid input is rejected by PVL preflight before any DHT read, so the
/// outcomes below apply only to input that already passed validation. Preflight therefore
/// outranks lookup failures, malformed stored candidates, `AlreadyPresent`, and `Conflict`.
///
/// Insertion identity is `source + target + relationship + occurrence`. The decision
/// compares decoded candidate fields (never raw tag bytes):
/// - no live link shares the identity            -> create link, `Inserted`
/// - identity + canonical key + rel props match  -> `AlreadyPresent` (target cache ignored)
/// - identity matches but key or rel props differ -> `Conflict` (no write)
///
/// Never creates a second live link with an identity that already exists.
pub fn put_smartlink(prepared: PreparedSmartLink) -> Result<PutSmartLinkOutcome, HolonError> {
    // Deliberately ahead of identity resolution; see the precedence note above.
    let validated_tag = prepare_validated_smartlink_tag(&prepared)?;

    // Candidates are filtered by relationship only — key is not part of identity, so
    // filtering by key would hide conflicts on a differing key.
    let candidates = expand_from_source(&prepared.source_id, &prepared.relationship_name)?;

    // Among identity matches, choose the lowest SmartLinkId for replay-stable behavior
    // independent of DHT query ordering.
    let identity_match = candidates
        .iter()
        .filter(|c| c.same_identity(&prepared))
        .min_by(|a, b| a.smartlink_id.0 .0.cmp(&b.smartlink_id.0 .0));

    if let Some(existing) = identity_match {
        if existing.is_already_present(&prepared) {
            return Ok(PutSmartLinkOutcome::AlreadyPresent(existing.smartlink_id.clone()));
        }
        return Ok(PutSmartLinkOutcome::Conflict(existing.smartlink_id.clone()));
    }

    Ok(PutSmartLinkOutcome::Inserted(create_prepared_smartlink(&prepared, validated_tag)?))
}

/// Commit-only variant of [`put_smartlink`] that reuses a successful relationship expansion.
///
/// The public storage API retains its complete standalone read/check/create behavior. Commit
/// orchestration owns this bounded optimization context and supplies it only while persisting its
/// relationship plan.
pub(crate) fn put_smartlink_cached(
    context: &mut SmartLinkWriteContext,
    prepared: PreparedSmartLink,
) -> Result<PutSmartLinkOutcome, HolonError> {
    // Preserve standalone preflight precedence even when a matching identity is already cached.
    let validated_tag = prepare_validated_smartlink_tag(&prepared)?;
    let bucket_key = SmartLinkBucketKey::from_prepared(&prepared);
    let identity = SmartLinkIdentity::from_prepared(&prepared);
    let bucket = context.bucket_for(bucket_key, || {
        expand_from_source(&prepared.source_id, &prepared.relationship_name)
    })?;

    if let Some(existing) = bucket.identities.get(&identity) {
        return Ok(existing.outcome_for(&prepared));
    }

    let smartlink_id = create_prepared_smartlink(&prepared, validated_tag)?;
    bucket.insert(identity, CachedSmartLink::from_inserted(&prepared, smartlink_id.clone()));
    Ok(PutSmartLinkOutcome::Inserted(smartlink_id))
}

// ---------------------------------------------------------------------------
// Delete: exact, idempotent
// ---------------------------------------------------------------------------

/// Deletes exactly the physical link named by `smartlink_id`, idempotently.
///
/// HDK `delete_link` is not self-reporting (a repeat delete just commits another
/// `DeleteLink`), and a `CreateLink` action's record details do **not** track its
/// `DeleteLink`s. Liveness is therefore established by reading the create-link
/// action to recover its base, then checking whether the link still appears among
/// that base's live links (`get_links` excludes deleted links). Deleting one id
/// never affects sibling live links.
pub fn delete_smartlink(smartlink_id: &SmartLinkId) -> Result<DeleteSmartLinkOutcome, HolonError> {
    let create_hash = try_action_hash_from_local_id(&smartlink_id.0)?;

    // Recover the base address from the create-link action (needed to test liveness).
    let Some(record) =
        get(create_hash.clone(), GetOptions::default()).map_err(holon_error_from_wasm_error)?
    else {
        return Ok(DeleteSmartLinkOutcome::AlreadyAbsent);
    };
    let base_address = match record.action() {
        Action::CreateLink(create_link) => create_link.base_address.clone(),
        _ => {
            return Err(HolonError::InvalidParameter(format!(
                "SmartLinkId {:?} does not reference a create-link action",
                smartlink_id
            )));
        }
    };

    // A link is live iff it still appears in its base's live links.
    let query = LinkQuery::try_new(base_address, LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?;
    let links = get_links(query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let live = links.iter().any(|link| link.create_link_hash == create_hash);
    if !live {
        return Ok(DeleteSmartLinkOutcome::AlreadyAbsent);
    }

    delete_link(create_hash, GetOptions::default()).map_err(holon_error_from_wasm_error)?;
    Ok(DeleteSmartLinkOutcome::Deleted)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Encodes and validates the exact Tag v1 bytes intended for `create_link`.
///
/// Coordinator preflight applies the shared pure PVL rules to canonical typed input. Integrity
/// still validates arbitrary peer bytes and Holochain operation facts, so passing this check does
/// not guarantee conductor acceptance. Unlike typed entry authoring, `create_link` accepts the
/// already encoded tag; returning the validated bytes ensures there is no re-encoding seam where
/// the submitted representation could drift.
///
/// Codec-only writer failures remain storage errors because they have no peer-visible PVL
/// equivalent. PVL validation begins only after the canonical storage encoder succeeds.
fn prepare_validated_smartlink_tag(prepared: &PreparedSmartLink) -> Result<Vec<u8>, HolonError> {
    let encoded = encode_smartlink_tag(&prepared.to_tag_input()).map_err(tag_error)?;

    validate_smartlink_envelope(&prepared.source_id, prepared.target_id.local_id(), &encoded)
        .map_err(HolonError::PvlViolation)?;

    Ok(encoded)
}

fn create_prepared_smartlink(
    prepared: &PreparedSmartLink,
    validated_tag: Vec<u8>,
) -> Result<SmartLinkId, HolonError> {
    let source_hash = try_action_hash_from_local_id(&prepared.source_id)?;
    let target_hash = try_action_hash_from_local_id(prepared.target_id.local_id())?;
    let create_hash =
        create_link(source_hash, target_hash, LinkTypes::SmartLink, LinkTag(validated_tag))
            .map_err(holon_error_from_wasm_error)?;
    Ok(SmartLinkId(local_id_from_action_hash(create_hash)))
}

/// Runs a prepared `LinkQuery` and decodes every live match into a `SmartLink`.
fn decode_query(source_id: &LocalId, query: LinkQuery) -> Result<Vec<SmartLink>, HolonError> {
    let links = get_links(query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        out.push(decode_link_to_smartlink(source_id, link)?);
    }
    Ok(out)
}

/// Decodes one Holochain `Link` into a storage `SmartLink`, preserving its physical id.
///
/// The tag decode (and its malformed → `InvalidWireFormat`-naming-the-id mapping) lives in
/// `core_types::decode_smartlink`; here we only extract the physical id and endpoints from the
/// `Link` and delegate.
fn decode_link_to_smartlink(source_id: &LocalId, link: Link) -> Result<SmartLink, HolonError> {
    let smartlink_id = SmartLinkId(local_id_from_action_hash(link.create_link_hash));
    let target = link
        .target
        .into_action_hash()
        .ok_or_else(|| malformed_link_error(&smartlink_id, "link target is not an action hash"))?;
    decode_smartlink(
        smartlink_id,
        source_id.clone(),
        local_id_from_action_hash(target),
        &link.tag.into_inner(),
    )
}

fn malformed_link_error(id: &SmartLinkId, error: impl std::fmt::Display) -> HolonError {
    HolonError::InvalidWireFormat {
        wire_type: "SmartLinkTagV1".to_string(),
        reason: format!("malformed SmartLink {:?}: {}", id, error),
    }
}

fn tag_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::InvalidWireFormat {
        wire_type: "SmartLinkTagV1".to_string(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::MapString;
    use core_types::{CanonicalKey, HolonId, PropertyMap, SmartLink, HOLOCHAIN_ACTION_HASH_BYTES};
    use integrity_core_types::PvlViolation;

    fn local_id(seed: u8) -> LocalId {
        LocalId(vec![seed; HOLOCHAIN_ACTION_HASH_BYTES])
    }

    fn prepared(relationship_name: impl Into<String>) -> PreparedSmartLink {
        PreparedSmartLink {
            source_id: local_id(1),
            target_id: HolonId::Local(local_id(2)),
            relationship_name: RelationshipName(MapString(relationship_name.into())),
            canonical_key: CanonicalKey::new("").unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        }
    }

    fn decoded_smartlink(id_seed: u8, canonical_key: &str) -> SmartLink {
        SmartLink {
            smartlink_id: SmartLinkId(local_id(id_seed)),
            source_id: local_id(1),
            target_id: HolonId::Local(local_id(2)),
            relationship_name: RelationshipName(MapString("Related".to_string())),
            canonical_key: CanonicalKey::new(canonical_key).unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_values: PropertyMap::new(),
        }
    }

    #[test]
    fn prepared_tag_maps_structural_failure_to_the_stable_pvl_error() {
        // An empty relationship name is encodable Tag v1 data, which proves this failure comes
        // from shared PVL validation rather than from the storage encoder.
        let error = prepare_validated_smartlink_tag(&prepared("")).unwrap_err();

        assert_eq!(error, HolonError::PvlViolation(PvlViolation::EmptyRelationshipName));
        assert_eq!(error.to_string(), "MAP-PVL-2101: relationship name is empty");
    }

    #[test]
    fn prepared_tag_keeps_packing_failure_as_a_storage_error() {
        let error = prepare_validated_smartlink_tag(&prepared("r".repeat(600))).unwrap_err();

        match error {
            HolonError::InvalidWireFormat { wire_type, reason } => {
                assert_eq!(wire_type, "SmartLinkTagV1");
                assert!(
                    reason.contains("mandatory SmartLink content")
                        && reason.contains("packing budget"),
                    "unexpected packing error: {reason}"
                );
            }
            other => panic!("packing failure must remain a storage error, got {other:?}"),
        }
    }

    #[test]
    fn write_context_expands_a_bucket_once_and_keeps_the_lowest_identity_match() {
        let prepared = prepared("Related");
        let bucket_key = SmartLinkBucketKey::from_prepared(&prepared);
        let identity = SmartLinkIdentity::from_prepared(&prepared);
        let mut context = SmartLinkWriteContext::default();
        let mut expansion_count = 0;

        {
            let bucket = context
                .bucket_for(bucket_key.clone(), || {
                    expansion_count += 1;
                    // The DHT does not promise ordering. The cache must retain the same lowest-id
                    // choice as standalone put_smartlink regardless of expansion order.
                    Ok(vec![
                        decoded_smartlink(9, "newer-conflicting-key"),
                        decoded_smartlink(3, ""),
                    ])
                })
                .unwrap();
            assert_eq!(
                bucket.identities.get(&identity).unwrap().outcome_for(&prepared),
                PutSmartLinkOutcome::AlreadyPresent(SmartLinkId(local_id(3)))
            );
        }

        let _ = context
            .bucket_for(bucket_key, || -> Result<Vec<SmartLink>, HolonError> {
                panic!("a populated commit bucket must not expand again")
            })
            .unwrap();
        assert_eq!(expansion_count, 1);
    }

    #[test]
    fn write_context_records_an_inserted_identity_for_later_writes() {
        let prepared = prepared("Related");
        let identity = SmartLinkIdentity::from_prepared(&prepared);
        let mut bucket = SmartLinkBucket::default();
        let inserted = SmartLinkId(local_id(7));

        bucket
            .insert(identity.clone(), CachedSmartLink::from_inserted(&prepared, inserted.clone()));

        assert_eq!(
            bucket.identities.get(&identity).unwrap().outcome_for(&prepared),
            PutSmartLinkOutcome::AlreadyPresent(inserted)
        );
    }
}
