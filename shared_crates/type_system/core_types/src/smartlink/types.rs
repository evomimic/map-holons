use crate::{HolonError, HolonId, LocalId, PropertyMap, PropertyName, RelationshipName};
use base_types::BaseValue;
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
}
