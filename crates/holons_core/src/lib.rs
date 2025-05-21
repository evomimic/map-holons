// Top-level modules exposed to external consumers
pub mod core_shared_objects;
pub mod reference_layer;
// Utility modules (if needed outside the crate)
pub mod dances;
pub mod query_layer;
pub mod utils;

// pub use core_shared_objects::*;
pub use core_shared_objects::{
    CollectionState, CommitRequestStatus, CommitResponse,
    HolonCache, HolonCacheAccess, HolonCacheManager, HolonCollection, HolonError, HolonPool,
    Nursery, NurseryAccess, RelationshipCache, RelationshipMap,
    RelationshipName, ServiceRoutingPolicy, StagedRelationshipMap, TransientCollection,
};
pub use reference_layer::holon_operations_api;
pub use reference_layer::{
    HolonCollectionApi, HolonReadable, HolonReference, HolonServiceApi, HolonSpaceBehavior,
    HolonStagingBehavior, HolonWritable, HolonsContextBehavior, SmartReference, StagedReference,
    TransientCollectionBehavior,
};
// pub use utils::*;
