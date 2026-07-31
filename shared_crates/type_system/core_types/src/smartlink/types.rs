use crate::{HolonError, HolonId, LocalId, PropertyMap, PropertyName, RelationshipName};
use base_types::{BaseValue, MapString};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Physical identity of one persisted directional SmartLink create-link action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SmartLinkId(pub LocalId);

impl From<LocalId> for SmartLinkId {
    fn from(value: LocalId) -> Self {
        Self(value)
    }
}

impl From<SmartLinkId> for LocalId {
    fn from(value: SmartLinkId) -> Self {
        value.0
    }
}

/// Canonical target key embedded in every SmartLink tag.
///
/// The empty string is valid for keyless targets. NUL is forbidden because it
/// terminates the key segment in the Tag v1 prefix grammar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CanonicalKey(String);

impl CanonicalKey {
    /// Constructs a canonical key from a Tag v1 delimiter-extracted UTF-8 segment.
    ///
    /// This is intentionally crate-private: the codec's successful delimiter
    /// scan proves that the segment contains no NUL, which discharges the
    /// invariant without repeating fallible validation on peer bytes.
    pub(crate) fn from_delimited_segment(value: String) -> Self {
        Self(value)
    }

    pub fn new(value: impl Into<String>) -> Result<Self, HolonError> {
        Self::try_from(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for CanonicalKey {
    type Error = HolonError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_nul_free("CanonicalKey", &value)?;
        Ok(Self(value))
    }
}

impl From<CanonicalKey> for String {
    fn from(value: CanonicalKey) -> Self {
        value.0
    }
}

impl fmt::Display for CanonicalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// NUL-free prefix used for canonical-key prefix retrieval.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CanonicalKeyPrefix(String);

impl CanonicalKeyPrefix {
    pub fn new(value: impl Into<String>) -> Result<Self, HolonError> {
        Self::try_from(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for CanonicalKeyPrefix {
    type Error = HolonError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_nul_free("CanonicalKeyPrefix", &value)?;
        Ok(Self(value))
    }
}

impl From<CanonicalKeyPrefix> for String {
    fn from(value: CanonicalKeyPrefix) -> Self {
        value.0
    }
}

impl fmt::Display for CanonicalKeyPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque semantic identity shared by both directions of one occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OccurrenceId(pub [u8; 16]);

/// Canonical-key selection supported by the SmartLink storage access paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyMatch {
    Exact(CanonicalKey),
    StartsWith(CanonicalKeyPrefix),
}

/// One optional target-property cache candidate in writer priority order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetPropertyCacheCandidate {
    pub property_name: PropertyName,
    pub value: BaseValue,
}

/// Codec input prepared by coordination for deterministic Tag v1 packing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartLinkTagInput {
    pub target_id: HolonId,
    pub relationship_name: RelationshipName,
    pub canonical_key: CanonicalKey,
    pub occurrence_id: Option<OccurrenceId>,
    pub relationship_property_values: PropertyMap,
    pub target_property_cache_candidates: Vec<TargetPropertyCacheCandidate>,
}

/// Information assembled from canonical SmartLink Tag v1 bytes and the
/// caller-supplied Holochain link-target identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSmartLinkTag {
    pub target_id: HolonId,
    pub relationship_name: RelationshipName,
    pub canonical_key: CanonicalKey,
    pub occurrence_id: Option<OccurrenceId>,
    pub relationship_property_values: PropertyMap,
    pub target_property_values: PropertyMap,
}

/// Fully-prepared, descriptor-unaware input to `put_smartlink`.
///
/// Coordination supplies every field; the storage layer never infers or derives
/// a missing one. `occurrence_id` is always `None` in SL1 Part 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSmartLink {
    pub source_id: LocalId,
    pub target_id: HolonId,
    pub relationship_name: RelationshipName,
    pub canonical_key: CanonicalKey,
    pub occurrence_id: Option<OccurrenceId>,
    pub relationship_property_values: PropertyMap,
    pub target_property_cache_candidates: Vec<TargetPropertyCacheCandidate>,
}

impl PreparedSmartLink {
    /// Builds the codec input for Tag v1 packing, reusing the shared encoder.
    pub fn to_tag_input(&self) -> SmartLinkTagInput {
        SmartLinkTagInput {
            target_id: self.target_id.clone(),
            relationship_name: self.relationship_name.clone(),
            canonical_key: self.canonical_key.clone(),
            occurrence_id: self.occurrence_id,
            relationship_property_values: self.relationship_property_values.clone(),
            target_property_cache_candidates: self.target_property_cache_candidates.clone(),
        }
    }
}

/// One decoded, unhydrated SmartLink returned by the storage expansion APIs.
///
/// Preserves the physical `SmartLinkId`, keeps the relationship-property and
/// target-property caches as separate maps, and round-trips the optional
/// `OccurrenceId` (always absent in SL1 Part 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartLink {
    pub smartlink_id: SmartLinkId,
    pub source_id: LocalId,
    pub target_id: HolonId,
    pub relationship_name: RelationshipName,
    pub canonical_key: CanonicalKey,
    pub occurrence_id: Option<OccurrenceId>,
    pub relationship_property_values: PropertyMap,
    pub target_property_values: PropertyMap,
}

impl SmartLink {
    /// Returns the tag's canonical key as a `MapString`, or `None` when keyless.
    pub fn key(&self) -> Result<Option<MapString>, HolonError> {
        Ok((!self.canonical_key.as_str().is_empty())
            .then(|| MapString(self.canonical_key.as_str().to_string())))
    }

    /// Returns the persisted target identity and any cached smart properties.
    /// This is context-free and does not mint runtime references.
    pub fn to_pointer(&self) -> (HolonId, Option<PropertyMap>) {
        (
            self.target_id.clone(),
            (!self.target_property_values.is_empty()).then(|| self.target_property_values.clone()),
        )
    }

    /// True when this live link shares `prepared`'s insertion identity:
    /// source + target + relationship + occurrence. Canonical key and property
    /// caches are deliberately excluded.
    pub fn same_identity(&self, prepared: &PreparedSmartLink) -> bool {
        self.source_id == prepared.source_id
            && self.target_id == prepared.target_id
            && self.relationship_name == prepared.relationship_name
            && self.occurrence_id == prepared.occurrence_id
    }

    /// True when this live link is authoritatively equal to `prepared`: identity
    /// matches AND canonical key and relationship-property values match. Optional
    /// target-property cache differences are ignored (idempotency requirement).
    pub fn is_already_present(&self, prepared: &PreparedSmartLink) -> bool {
        self.same_identity(prepared)
            && self.canonical_key == prepared.canonical_key
            && self.relationship_property_values == prepared.relationship_property_values
    }
}

/// Decodes one persisted SmartLink's Tag v1 `tag_bytes` into a storage `SmartLink`,
/// pairing it with its physical identity and link endpoints.
///
/// On a malformed tag, fails with `HolonError::InvalidWireFormat` whose `reason` names
/// the offending `SmartLinkId` — the storage layer's "fail the whole expansion and
/// identify the bad link" contract. A malformed *live* link is unreachable through the
/// DHT (SmartLink integrity validation decodes the tag with this same codec at create
/// time), so this failure path is exercised by unit tests rather than a conductor.
pub fn decode_smartlink(
    smartlink_id: SmartLinkId,
    source_id: LocalId,
    link_target: LocalId,
    tag_bytes: &[u8],
) -> Result<SmartLink, HolonError> {
    let decoded = super::decode_smartlink_tag(tag_bytes, link_target).map_err(|error| {
        HolonError::InvalidWireFormat {
            wire_type: "SmartLinkTagV1".to_string(),
            reason: format!("malformed SmartLink {smartlink_id:?}: {error}"),
        }
    })?;
    Ok(SmartLink {
        smartlink_id,
        source_id,
        target_id: decoded.target_id,
        relationship_name: decoded.relationship_name,
        canonical_key: decoded.canonical_key,
        occurrence_id: decoded.occurrence_id,
        relationship_property_values: decoded.relationship_property_values,
        target_property_values: decoded.target_property_values,
    })
}

/// Outcome of `put_smartlink`. Each variant carries the physical id of the
/// live link the decision concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PutSmartLinkOutcome {
    /// A new physical link was created.
    Inserted(SmartLinkId),
    /// An authoritatively-identical live link already existed; no write.
    AlreadyPresent(SmartLinkId),
    /// A live link shares the insertion identity but differs in canonical key or
    /// authoritative relationship properties; no write.
    Conflict(SmartLinkId),
}

/// Outcome of `delete_smartlink`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteSmartLinkOutcome {
    /// A live link existed and was deleted by this call.
    Deleted,
    /// No live link existed for the id (already deleted or never present).
    AlreadyAbsent,
}

fn validate_nul_free(wire_type: &str, value: &str) -> Result<(), HolonError> {
    if value.as_bytes().contains(&0) {
        return Err(HolonError::InvalidWireFormat {
            wire_type: wire_type.to_string(),
            reason: "value contains a NUL byte".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_accepts_empty_and_rejects_nul() {
        assert_eq!(CanonicalKey::new("").unwrap().as_str(), "");
        assert!(matches!(
            CanonicalKey::new("key\0suffix"),
            Err(HolonError::InvalidWireFormat { .. })
        ));
    }

    #[test]
    fn canonical_key_prefix_rejects_nul() {
        assert!(matches!(
            CanonicalKeyPrefix::new("prefix\0"),
            Err(HolonError::InvalidWireFormat { .. })
        ));
    }

    fn local(seed: u8) -> LocalId {
        LocalId(vec![seed; 4])
    }

    fn rel(name: &str) -> RelationshipName {
        RelationshipName(MapString(name.to_string()))
    }

    fn prop(name: &str, value: &str) -> PropertyMap {
        std::collections::BTreeMap::from([(
            PropertyName(MapString(name.to_string())),
            BaseValue::StringValue(MapString(value.to_string())),
        )])
    }

    fn prepared() -> PreparedSmartLink {
        PreparedSmartLink {
            source_id: local(1),
            target_id: HolonId::Local(local(2)),
            relationship_name: rel("RelatedTo"),
            canonical_key: CanonicalKey::new("target-key").unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        }
    }

    /// Builds a live decoded link that authoritatively matches `p` by default.
    fn live_from(p: &PreparedSmartLink, id: u8) -> SmartLink {
        SmartLink {
            smartlink_id: SmartLinkId(local(id)),
            source_id: p.source_id.clone(),
            target_id: p.target_id.clone(),
            relationship_name: p.relationship_name.clone(),
            canonical_key: p.canonical_key.clone(),
            occurrence_id: p.occurrence_id,
            relationship_property_values: p.relationship_property_values.clone(),
            target_property_values: PropertyMap::new(),
        }
    }

    #[test]
    fn same_identity_ignores_key_and_props() {
        let p = prepared();
        let mut live = live_from(&p, 9);
        live.canonical_key = CanonicalKey::new("different-key").unwrap();
        live.relationship_property_values = prop("weight", "heavy");
        assert!(live.same_identity(&p));
    }

    #[test]
    fn same_identity_false_on_different_target() {
        let p = prepared();
        let mut live = live_from(&p, 9);
        live.target_id = HolonId::Local(local(7));
        assert!(!live.same_identity(&p));
    }

    #[test]
    fn already_present_when_identity_key_and_props_match() {
        let p = prepared();
        let live = live_from(&p, 9);
        assert!(live.is_already_present(&p));
    }

    #[test]
    fn already_present_ignores_target_property_cache() {
        let p = prepared();
        let mut live = live_from(&p, 9);
        live.target_property_values = prop("title", "cached"); // cache-only difference
        assert!(live.is_already_present(&p));
    }

    #[test]
    fn conflict_when_canonical_key_differs() {
        let p = prepared();
        let mut live = live_from(&p, 9);
        live.canonical_key = CanonicalKey::new("other-key").unwrap();
        assert!(live.same_identity(&p));
        assert!(!live.is_already_present(&p)); // -> Conflict
    }

    #[test]
    fn conflict_when_relationship_props_differ() {
        let mut p = prepared();
        p.relationship_property_values = prop("weight", "light");
        let mut live = live_from(&p, 9);
        live.relationship_property_values = prop("weight", "heavy");
        assert!(live.same_identity(&p));
        assert!(!live.is_already_present(&p)); // -> Conflict
    }

    /// 39-byte action-hash-shaped id, required by the codec's hash validation.
    fn hash39(seed: u8) -> LocalId {
        let mut bytes = vec![seed; 39];
        bytes[7] = 0;
        LocalId(bytes)
    }

    #[test]
    fn decode_smartlink_round_trips_valid_tag() {
        let input = SmartLinkTagInput {
            target_id: HolonId::Local(hash39(2)),
            relationship_name: rel("Likes"),
            canonical_key: CanonicalKey::new("apple").unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        };
        let bytes = super::super::encode_smartlink_tag(&input).unwrap();

        let sl = decode_smartlink(SmartLinkId(hash39(9)), hash39(1), hash39(2), &bytes).unwrap();
        assert_eq!(sl.smartlink_id, SmartLinkId(hash39(9)));
        assert_eq!(sl.source_id, hash39(1));
        assert_eq!(sl.target_id, HolonId::Local(hash39(2)));
        assert_eq!(sl.relationship_name, rel("Likes"));
        assert_eq!(sl.canonical_key, CanonicalKey::new("apple").unwrap());
    }

    #[test]
    fn decode_smartlink_malformed_names_offending_id() {
        let id = SmartLinkId(hash39(9));
        // Junk bytes are not a valid Tag v1 payload; the link target is valid so the
        // failure is attributed to the tag.
        let err = decode_smartlink(id.clone(), hash39(1), hash39(2), &[0u8, 1, 2, 3]).unwrap_err();
        match err {
            HolonError::InvalidWireFormat { wire_type, reason } => {
                assert_eq!(wire_type, "SmartLinkTagV1");
                assert!(
                    reason.contains(&format!("{id:?}")),
                    "reason must name the offending SmartLinkId, got: {reason}"
                );
            }
            other => panic!("expected InvalidWireFormat, got {other:?}"),
        }
    }
}
