//! Versioned prelude (v1). Mirrors the current crate root. Stable for consumers.

pub use crate::{
    // Core shared objects you currently re-export at root:
    CollectionState,
    CommitRequestStatus,
    CommitResponse,
    HolonCache,
    HolonCacheAccess,
    HolonCacheManager,
    HolonCollection,
    HolonPool,
    HolonReference,
    HolonServiceApi,
    HolonSpaceBehavior,
    HolonStagingBehavior,
    HolonsContextBehavior,
    Nursery,
    NurseryAccess,
    // Facade traits and key types you already export at the root today:
    ReadableHolon,
    RelationshipCache,
    RelationshipMap,
    ServiceRoutingPolicy,
    SmartReference,
    StagedReference,
    StagedRelationshipMap,
    TransientCollection,
    TransientCollectionBehavior,

    WritableHolon,
};

// If you also want error/base types in the prelude:
pub use core_types::HolonError;
// Converters:
pub use type_names::relationship_names::ToRelationshipName;
pub use type_names::ToPropertyName;
