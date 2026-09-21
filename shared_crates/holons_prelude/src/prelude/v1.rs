//! Versioned prelude (v1). Stable for consumers.
//! # Holons Prelude (v1)
//!
//! The curated public API surface for the MAP Holons Layer (L0).
//!
//! This prelude provides convenient, stable access to core types, values, APIs,
//! and request builders across the following shared_crates:
//!
//! - `base_types` – scalar wrapper types like `MapString` and `MapBoolean`
//! - `core_types` – core identifiers like `HolonId`
//! - `integrity_core_types` – property/relationship names, errors, value wrappers
//! - `holon_dance_builders` – builder functions for each `DanceRequest`
//! - `holons_core` – core `Holon`, `DanceRequest`, `Query`, and reference layer APIs
//! - `type_names` – constants and traits for working with core type and property names
//!
//! ## Stability
//!
//! This is **versioned as `v1`** to support long-term stability and backwards compatibility
//! for downstream consumers. Future versions (`v2`, etc.) may evolve as the MAP architecture grows.
//!
//! ## Not Included
//!
//! - Internal utilities or experimental types
//! - Any Holochain-specific tracing setup
//!
//! To opt into tracing, use your own `tracing_subscriber` config — this prelude is agnostic.
//!
//! ## When to Use
//!
//! - ✅ Application developers using MAP L0 APIs
//! - ✅ Tests that need to construct or inspect holons
//! - ✅ Tooling or CLI layers that construct `DanceRequest`s
//! - ❌ Internal modules within `holons_core` (should import directly)

pub use base_types::{
    BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString, ToBaseValue,
};
pub use core_types::HolonId;
pub use integrity_core_types::{
    HolonError, HolonErrorKind, PropertyMap, PropertyName, PropertyValue, RelationshipName,
};

pub use holon_dance_builders::*;
pub use holons_core::core_shared_objects::holon::state::AccessType;
pub use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle,
};
pub use holons_core::core_shared_objects::{HolonCollection, RelationshipMap};
pub use holons_core::dances::{
    DanceInitiator, DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode,
};
pub use holons_core::query_layer::{Node, NodeCollection, QueryExpression, QueryPathMap};
pub use holons_core::reference_layer::{
    CompletionOutcome, Divergence, EquivalenceOutcome, EquivalenceResolver, HolonCollectionApi,
    HolonReference, HolonSpaceBehavior, HolonStagingBehavior, NoOpResolver, NodeResolution,
    ReadableHolon, SmartReference, StagedReference, TransientHolonBehavior, TransientReference,
    WritableHolon,
};
pub use holons_core::{
    ContractContributions, DescribingCompatibility, DescribingTypeResolution, Descriptor,
    DescriptorKindRoots, EffectiveRelationshipMember, ExtendsIter, ExtendsLineageDefect,
    ExtendsLineageDiagnosis, HolonDescriptor, HolonSpaceDescriptor, PropertyDescriptor,
    ProspectiveIdentity, RelationshipDescriptor, RelationshipDirection, ResolvedValueTypeRoots,
    SchemaOwnershipKind, SchemaOwnershipResolution, StructuralPrerequisites, TransactionDescriptor,
    TypeHeader, UniversalDescriptorContract, ValidExtendsLineage, ValueDescriptor,
    ValueDescriptorKind, ancestors, applicable_descriptor_types,
    assert_reference_transaction_compatible, classify_relationship_direction,
    constraint_applies_to, effective_relationship_declaration, effective_relationship_targets,
    equals_or_extends, resolve_core_descriptor, resolve_describing_type, resolve_schema_ownership,
    schema_components, schema_dependencies, schema_rules, walk_extends_chain,
};

pub use type_names::{
    CommandName, CoreCommandTypeName, CoreHolonTypeName, CoreOperatorTypeName,
    CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName, OperatorName, ToCommandName,
    ToOperatorName, ToPropertyName, ToRelationshipName,
};
