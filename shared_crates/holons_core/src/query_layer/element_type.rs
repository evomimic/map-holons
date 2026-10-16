//! Declared collection type for QueryCore.
//!
//! Every operand and result collection declares exactly one `ElementType`: the
//! holon type its members conform to. Query selectors resolve against that
//! declared type's effective surface, including inherited declarations, and
//! never against the members' own descriptors. Members may be instances of
//! subtypes, but a subtype's own declarations neither enlarge nor rebind the
//! selectable surface. Member conformance is a data-lifecycle invariant; the
//! query engine does not re-establish it by scanning members.

use core_types::{HolonError, RelationshipName};
use type_names::CoreRelationshipTypeName;

use crate::descriptors::accessor_helpers::require_single_related;
use crate::descriptors::HolonDescriptor;
use crate::reference_layer::HolonReference;

/// The declared element type of `collection`: its single `ElementType` target.
///
/// A missing declaration is `MissingRequiredRelationship`; several are
/// `MultipleRelatedHolons`.
pub(crate) fn declared_element_type(
    collection: &HolonReference,
) -> Result<HolonDescriptor, HolonError> {
    Ok(HolonDescriptor::from_holon(require_single_related(
        collection,
        CoreRelationshipTypeName::ElementType,
    )?))
}

/// The element type of navigating `relationship_name` from instances of
/// `element_type`: the target type of the declared or inverse relationship it
/// resolves to. Resolution errors (an unavailable or ambiguous name, an
/// unsaved descriptor endpoint) propagate unchanged.
pub(crate) fn expansion_target_type(
    element_type: &HolonDescriptor,
    relationship_name: &RelationshipName,
) -> Result<HolonDescriptor, HolonError> {
    element_type.resolve_available_relationship(relationship_name.clone())?.descriptor.target_type()
}
