//! Representation-neutral MAP descriptor semantics.
//!
//! This crate owns graph and conformance rules that must behave identically for bound runtime
//! holons and Canonical Holon IR. Representations provide graph access; this crate owns traversal,
//! cardinality, ordering, deduplication, and value-policy decisions. It deliberately has no
//! storage, transaction, runtime-reference, source-format, or host dependency.

pub mod conformance;
pub mod graph;
pub mod inheritance;
pub mod value;

pub use conformance::{
    compose_restrictive_boolean, validate_cardinality, validate_holon_conformance,
    value_policy_for_type_kind, CardinalityViolation, ConformanceValue, ConformanceViolation,
    HolonConformance, PropertyDeclaration, PropertyValue, RelationshipDeclaration,
    RelationshipValue, ValuePolicy,
};
pub use graph::{DescriptorGraph, DescriptorSemanticsError};
pub use inheritance::{
    ancestors, collect_named_members_from_lineage, describing_type, duplicate_declaration_name,
    effective_descriptor_lineage, effective_holon_key_rule, effective_instance_key_rule,
    equals_or_extends, flatten_named_members, flatten_related_members, lineage, walk_extends_chain,
    ExtendsTraversal, ExtendsWalk,
};
pub use value::{
    validate_enum_variant, validate_integer_maximum, validate_integer_minimum,
    validate_string_maximum_length, validate_string_minimum_length, ValueViolation,
};
