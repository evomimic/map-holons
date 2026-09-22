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
    Nursery, NurseryAccess, RelationshipCache, RelationshipCachePolicy, RelationshipMap,
    RelationshipReadHint, ServiceRoutingPolicy, StagedRelationshipMap, TransientCollection,
};
pub use core_types::HolonError;
pub use descriptors::{
    ancestors, applicable_descriptor_types, classify_relationship_direction, constraint_applies_to,
    effective_relationship_declaration, effective_relationship_targets, equals_or_extends,
    resolve_core_descriptor, resolve_describing_type, resolve_schema_ownership, same_definition,
    schema_components, schema_dependencies, schema_rules, walk_extends_chain, AssessmentReadError,
    ContractContributions, CurrentDescriptorReader, DescribingCompatibility,
    DescribingTypeResolution, Descriptor, DescriptorKindRoots, DescriptorReader,
    EffectiveRelationshipMember, ExtendsIter, ExtendsLineageDefect, ExtendsLineageDiagnosis,
    HolonDescriptor, HolonSpaceDescriptor, PropertyDescriptor, ProspectiveDescriptorReader,
    ProspectiveSelection, RelationshipDescriptor, RelationshipDirection, ResolvedValueTypeRoots,
    SchemaOwnershipKind, SchemaOwnershipResolution, StructuralPrerequisites, TransactionDescriptor,
    TypeHeader, UniversalDescriptorContract, ValidExtendsLineage, ValueDescriptor,
    ValueDescriptorKind,
};
pub use reference_layer::{
    assert_reference_transaction_compatible, CompletionOutcome, Divergence, EquivalenceOutcome,
    EquivalenceResolver, HolonCollectionApi, HolonReference, HolonServiceApi, HolonSpaceBehavior,
    HolonStagingBehavior, NoOpResolver, NodeResolution, ProspectiveIdentity, ReadableHolon,
    SmartReference, StagedReference, TransientHolonBehavior, TransientReference, WritableHolon,
};
// pub use utils::*;
