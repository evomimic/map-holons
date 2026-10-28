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
    constraint_applies_to_lineage, effective_relationship_declaration,
    effective_relationship_targets, equals_or_extends, extends_role, has_role,
    resolve_core_descriptor, resolve_describing_type, resolve_schema_ownership, same_definition,
    schema_components, schema_dependencies, schema_rules, walk_extends_chain,
    ContractContributions, DescribingCompatibility, DescribingTypeResolution, Descriptor,
    DescriptorKindSemantics, EffectiveRelationshipMember, ExtendsIter, ExtendsLineageDefect,
    ExtendsLineageDiagnosis, HolonDescriptor, HolonSpaceDescriptor, PropertyDescriptor,
    RelationshipDescriptor, RelationshipDirection, RequiredCategory, SchemaOwnershipKind,
    SchemaOwnershipResolution, StructuralPrerequisites, TransactionDescriptor, TypeHeader,
    UniversalDescriptorContract, ValidExtendsLineage, ValueDescriptor, ValueDescriptorKind,
};
pub use reference_layer::{
    assert_reference_transaction_compatible, DefinitionIdentity, Divergence, EquivalenceOutcome,
    EquivalenceResolver, HolonCollectionApi, HolonReference, HolonServiceApi, HolonSpaceBehavior,
    HolonStagingBehavior, LineageIdentity, NoOpResolver, NodeResolution, ReadableHolon,
    SmartReference, StagedReference, TransientHolonBehavior, TransientReference, WritableHolon,
};
// pub use utils::*;
