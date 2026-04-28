pub(crate) mod accessor_helpers;
pub mod declared_relationship_descriptor;
pub mod descriptor;
pub mod holon_descriptor;
pub mod inheritance;
pub mod inverse_relationship_descriptor;
pub mod property_descriptor;
pub mod relationship_descriptor;
#[cfg(test)]
mod schema_contract_tests;
#[cfg(test)]
pub(crate) mod test_support;
pub mod type_header;
pub mod value_descriptor;

pub use declared_relationship_descriptor::DeclaredRelationshipDescriptor;
pub use descriptor::Descriptor;
pub use holon_descriptor::HolonDescriptor;
pub use inheritance::{ancestors, walk_extends_chain, ExtendsIter};
pub use inverse_relationship_descriptor::InverseRelationshipDescriptor;
pub use property_descriptor::PropertyDescriptor;
pub use relationship_descriptor::RelationshipDescriptor;
pub use type_header::TypeHeader;
pub use value_descriptor::ValueDescriptor;
