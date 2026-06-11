//! holons_core crate.
//!
//! Most users should import the prelude for the curated public API:
//! ```ignore
//! use holons_prelude::prelude::*;
//! ```

// Public Modules
pub mod core_shared_objects;
pub mod descriptors;
pub mod query_layer;
pub mod reference_layer;
// Utility modules (if needed outside the crate)
pub mod dances;
pub mod utils;

// pub use core_shared_objects::*;
pub use core_shared_objects::{
    CollectionState, HolonCache, HolonCacheAccess, HolonCacheManager, HolonCollection, HolonPool,
    Nursery, NurseryAccess, RelationshipCache, RelationshipMap, ServiceRoutingPolicy,
    StagedRelationshipMap, TransientCollection,
};
pub use core_types::HolonError;
pub use descriptors::{
    ancestors, classify_relationship_direction, effective_descriptor_lineage,
    effective_relationship_declaration, walk_extends_chain, Descriptor, ExtendsIter,
    HolonDescriptor, HolonSpaceDescriptor, PropertyDescriptor, RelationshipDescriptor,
    RelationshipDirection, TransactionDescriptor, TypeHeader, ValueDescriptor,
};
pub use reference_layer::{
    HolonCollectionApi, HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    ReadableHolon, SmartReference, StagedReference, TransientHolonBehavior, TransientReference,
    WritableHolon,
};
// pub use utils::*;
