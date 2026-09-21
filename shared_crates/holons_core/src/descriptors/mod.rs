pub(crate) mod accessor_helpers;
pub mod command_descriptor;
mod constraint_applicability;
mod contract_contributions;
pub mod dance_descriptor;
pub mod dance_response_descriptor;
pub mod declared_relationship_descriptor;
mod definition_identity;
pub mod descriptor;
pub mod effective_relationships;
pub mod holon_descriptor;
pub mod holon_space_descriptor;
pub mod inheritance;
pub mod inverse_relationship_descriptor;
pub mod inverse_resolution;
pub mod key_rule_descriptor;
mod kind_semantics;
pub mod operator_category;
pub mod operator_descriptor;
pub mod property_descriptor;
pub mod relationship_descriptor;
mod resolved_descriptor_roots;
#[cfg(test)]
mod schema_contract_tests;
mod schema_ownership;
#[cfg(test)]
mod semantic_products_tests;
mod structural_diagnosis;
#[cfg(test)]
pub(crate) mod test_support;
pub mod transaction_descriptor;
pub mod type_header;
mod universal_descriptor_contract;
pub mod value_descriptor;
pub mod value_descriptor_subtypes;

pub use command_descriptor::CommandDescriptor;
pub use constraint_applicability::{applicable_descriptor_types, constraint_applies_to};
pub use contract_contributions::ContractContributions;
pub use dance_descriptor::DanceDescriptor;
pub use dance_response_descriptor::DanceResponseDescriptor;
pub use declared_relationship_descriptor::DeclaredRelationshipDescriptor;
pub use descriptor::Descriptor;
pub use effective_relationships::{effective_relationship_declaration, QualifiedRelationship};
pub use holon_descriptor::HolonDescriptor;
pub use holon_space_descriptor::HolonSpaceDescriptor;
pub use inheritance::{
    ancestors, classify_relationship_direction, effective_relationship_targets, equals_or_extends,
    walk_extends_chain, EffectiveRelationshipMember, ExtendsIter, RelationshipDirection,
};
pub use inverse_relationship_descriptor::InverseRelationshipDescriptor;
pub use inverse_resolution::resolve_inverse_relationship_name;
pub use key_rule_descriptor::KeyRuleDescriptor;
pub use kind_semantics::{DescribingCompatibility, DescriptorKindRoots};
pub use operator_category::OperatorCategory;
pub use operator_descriptor::OperatorDescriptor;
pub use property_descriptor::PropertyDescriptor;
pub use relationship_descriptor::{EffectiveCardinality, RelationshipDescriptor, TargetBinding};
pub use resolved_descriptor_roots::{resolve_core_descriptor, ResolvedValueTypeRoots};
pub use schema_ownership::{
    resolve_schema_ownership, schema_components, schema_dependencies, schema_rules,
    SchemaOwnershipKind, SchemaOwnershipResolution,
};
pub use structural_diagnosis::{
    resolve_describing_type, DescribingTypeResolution, ExtendsLineageDefect,
    ExtendsLineageDiagnosis, StructuralPrerequisites, ValidExtendsLineage,
};
pub use transaction_descriptor::TransactionDescriptor;
pub use type_header::TypeHeader;
pub use universal_descriptor_contract::UniversalDescriptorContract;
pub use value_descriptor::{ValueDescriptor, ValueDescriptorKind};
pub use value_descriptor_subtypes::{
    EnumValueDescriptor, IntegerValueDescriptor, StringValueDescriptor, ValueArrayDescriptor,
};
