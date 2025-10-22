//! holons_core crate.
//!
//! Most users should import the prelude for the curated public API:
//! ```ignore
//! use holons_prelude::prelude::*;
//! ```

// Public Modules
pub mod core_shared_objects;
pub mod reference_layer;
// Utility modules (if needed outside the crate)
pub mod dances;
pub mod query_layer;
pub mod utils;

// pub use core_shared_objects::*;
pub use core_shared_objects::{
    CollectionState, CommitRequestStatus, CommitResponse, HolonCache, HolonCacheAccess,
    HolonCacheManager, HolonCollection, HolonPool, Nursery, NurseryAccess, RelationshipCache,
    RelationshipMap, ServiceRoutingPolicy, StagedRelationshipMap, TransientCollection,
};
pub use reference_layer::holon_operations_api::*;
pub use reference_layer::{
    HolonCollectionApi, HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    HolonsContextBehavior, ReadableHolon, SmartReference, StagedReference, TransientHolonBehavior,
    WritableHolon,
};
// pub use utils::*;

//temp
pub mod mock_conductor;
pub use mock_conductor::*;