//! Versioned prelude (v1). Mirrors the current crate root. Stable for consumers.

// pub use crate::{
//     // Core shared objects you currently re-export at root:
//     CollectionState,
//     CommitRequestStatus,
//     CommitResponse,
//     HolonCache,
//     HolonCacheAccess,
//     HolonCacheManager,
//     HolonCollection,
//     HolonPool,
//     HolonReference,
//     HolonServiceApi,
//     HolonSpaceBehavior,
//     HolonStagingBehavior,
//     HolonsContextBehavior,
//     Nursery,
//     NurseryAccess,
//     // Facade traits and key types you already export at the root today:
//     ReadableHolon,
//     RelationshipMap,
//     SmartReference,
//     StagedReference,
//     StagedRelationshipMap,
//     TransientCollectionBehavior,
//     WritableHolon,
// };
pub use crate::core_shared_objects::{
    CollectionState, CommitRequestStatus, CommitResponse, HolonCollection, RelationshipMap,
    StagedRelationshipMap, TransientCollection,
};
pub use crate::reference_layer::holon_operations_api::*;
pub use crate::reference_layer::{
    HolonCollectionApi, HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    HolonsContextBehavior, ReadableHolon, SmartReference, StagedReference,
    TransientCollectionBehavior, TransientHolonBehavior,
};

// If you also want error/base types in the prelude:
pub use core_types::HolonError;
