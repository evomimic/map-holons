//! Membership and its schema association cross the retrieval boundary together.
use crate::{HolonCollection, RelationshipCachePolicy};
use core_types::HolonId;
use serde::{Deserialize, Serialize};

/// Exact relationship version and its already-derived retention policy.
/// Contains values only; consuming it never resolves references or performs I/O.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipPolicyBinding {
    pub descriptor_id: HolonId,
    pub policy: RelationshipCachePolicy,
}

/// Retrieved membership with an optional authoritative descriptor association.
/// Request-local providers and foundational edges may use their local policy instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchedRelationship {
    pub collection: HolonCollection,
    pub descriptor: Option<RelationshipPolicyBinding>,
}
