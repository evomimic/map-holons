pub(crate) mod accessor_helpers;
pub mod command_descriptor;
pub mod dance_descriptor;
pub mod dance_response_descriptor;
pub mod declared_relationship_descriptor;
pub mod descriptor;
pub mod holon_descriptor;
pub mod holon_space_descriptor;
pub mod inheritance;
pub mod inverse_relationship_descriptor;
pub mod inverse_resolution;
pub mod key_rule_descriptor;
pub mod operator_category;
pub mod operator_descriptor;
pub mod property_descriptor;
pub mod relationship_descriptor;
pub mod relationship_normalization;
pub mod relationship_surface;
pub mod relationship_traversal;
#[cfg(test)]
mod schema_contract_tests;
#[cfg(test)]
pub(crate) mod test_support;
pub mod transaction_descriptor;
pub mod type_header;
pub mod value_descriptor;
pub mod value_descriptor_subtypes;

pub use command_descriptor::CommandDescriptor;
pub use dance_descriptor::DanceDescriptor;
pub use dance_response_descriptor::DanceResponseDescriptor;
pub use declared_relationship_descriptor::DeclaredRelationshipDescriptor;
pub use descriptor::Descriptor;
pub use holon_descriptor::HolonDescriptor;
pub use holon_space_descriptor::HolonSpaceDescriptor;
pub use inheritance::{
    ancestors, classify_relationship_direction, effective_descriptor_lineage, walk_extends_chain,
    ExtendsIter, RelationshipDirection,
};
pub use inverse_relationship_descriptor::InverseRelationshipDescriptor;
pub use inverse_resolution::resolve_inverse_relationship_name;
pub use key_rule_descriptor::KeyRuleDescriptor;
pub use operator_category::OperatorCategory;
pub use operator_descriptor::OperatorDescriptor;
pub use property_descriptor::PropertyDescriptor;
pub use relationship_descriptor::RelationshipDescriptor;
pub use relationship_surface::effective_relationship_declaration;
pub use relationship_traversal::{QualifiedRelationship, TraversalDirection};
pub use transaction_descriptor::TransactionDescriptor;
pub use type_header::TypeHeader;
pub use value_descriptor::ValueDescriptor;
pub use value_descriptor_subtypes::{
    EnumValueDescriptor, IntegerValueDescriptor, StringValueDescriptor, ValueArrayDescriptor,
};
