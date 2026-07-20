use crate::descriptors::accessor_helpers::{descriptor_label, lock_error, search_extends_chain};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use descriptor_semantics::{
    DescriptorGraph, DescriptorSemanticsError, ExtendsTraversal as SemanticExtendsTraversal,
};
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

/// Direction of a relationship type descriptor relative to its declared edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipDirection {
    /// The descriptor names the canonical source-to-target relationship.
    Declared,
    /// The descriptor names the inverse target-to-source relationship.
    Inverse,
}

struct HolonReferenceGraph;

impl DescriptorGraph for HolonReferenceGraph {
    type Node = HolonReference;
    type Identity = String;
    type MemberKind = CoreRelationshipTypeName;
    type Error = HolonError;

    fn identity(&self, node: &Self::Node) -> Self::Identity {
        node.reference_id_string()
    }

    fn extends_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
        related_members(node, CoreRelationshipTypeName::Extends)
    }

    fn described_by_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
        related_members(node, CoreRelationshipTypeName::DescribedBy)
    }

    fn related_members(
        &self,
        node: &Self::Node,
        member_kind: &Self::MemberKind,
    ) -> Result<Vec<Self::Node>, Self::Error> {
        related_members(node, member_kind.clone())
    }

    fn is_type_descriptor(&self, node: &Self::Node) -> Result<bool, Self::Error> {
        is_type_descriptor_descriptor(node)
    }
}

fn related_members(
    holon: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection_arc = holon.related_holons(relationship_name)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    Ok(collection.get_members().to_vec())
}

fn map_semantics_error(error: DescriptorSemanticsError<HolonError, HolonReference>) -> HolonError {
    match error {
        DescriptorSemanticsError::Access(error) => error,
        DescriptorSemanticsError::MultipleExtends { descriptor, count } => {
            HolonError::MultipleExtends { descriptor: descriptor_label(&descriptor), count }
        }
        DescriptorSemanticsError::MultipleDescribedBy { .. } => HolonError::DuplicateError(
            "DescribedBy".into(),
            "Expected exactly one descriptor target".into(),
        ),
        DescriptorSemanticsError::MissingDescribedBy { holon } => {
            HolonError::MissingRequiredRelationship {
                relationship: "DescribedBy".to_string(),
                descriptor: descriptor_label(&holon),
            }
        }
        DescriptorSemanticsError::CyclicExtends { descriptor } => {
            HolonError::CyclicExtends { descriptor: descriptor_label(&descriptor) }
        }
        DescriptorSemanticsError::MultipleRelatedMembers { descriptor, kind, count } => {
            HolonError::MultipleRelatedHolons {
                relationship: kind.to_string(),
                descriptor: descriptor_label(&descriptor),
                count,
            }
        }
        DescriptorSemanticsError::DuplicateInheritedDeclaration { descriptor, kind, name } => {
            HolonError::DuplicateInheritedDeclaration {
                kind: kind.to_string(),
                name,
                descriptor: descriptor_label(&descriptor),
            }
        }
    }
}

/// Returns a lazy iterator over the effective `Extends` chain.
///
/// Iteration always yields `self` first, then successive parents. Structural
/// errors are reported at the point the iterator discovers them so callers can
/// stop early once they have enough context.
pub fn walk_extends_chain(start: &HolonReference) -> ExtendsIter {
    ExtendsIter::new(start)
}

/// Materializes the full `Extends` chain including `self`.
///
/// This is the eager helper for callers that truly need the whole lineage
/// rather than a short-circuiting iterator walk.
pub fn ancestors(start: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    walk_extends_chain(start).collect()
}

/// Returns true when `candidate` is `anchor` or inherits from it through `Extends`.
///
/// Compatibility is based on reference identity, not descriptor names.
pub(crate) fn equals_or_extends(
    candidate: &HolonReference,
    anchor: &HolonReference,
) -> Result<bool, HolonError> {
    descriptor_semantics::equals_or_extends(&HolonReferenceGraph, candidate, anchor)
        .map_err(map_semantics_error)
}

/// Computes the effective descriptor lineage for a holon(per MAP Type System) v1.2.
///
/// Ordinary instances use the `Extends` lineage of their `DescribedBy` descriptor.
/// Descriptor holons additionally contribute their own `Extends` lineage before
/// the effective lineage of their describing `TypeDescriptor` descriptor.
pub fn effective_descriptor_lineage(
    holon: &HolonReference,
) -> Result<Vec<HolonReference>, HolonError> {
    descriptor_semantics::effective_descriptor_lineage(&HolonReferenceGraph, holon)
        .map_err(map_semantics_error)
}

pub(crate) fn described_by_descriptor(
    holon: &HolonReference,
) -> Result<Option<HolonReference>, HolonError> {
    let targets = HolonReferenceGraph.described_by_targets(holon)?;
    match targets.as_slice() {
        [] => Ok(None),
        [descriptor] => Ok(Some(descriptor.clone())),
        _ => Err(HolonError::DuplicateError(
            "DescribedBy".into(),
            "Expected exactly one descriptor target".into(),
        )),
    }
}

fn is_type_descriptor_descriptor(descriptor: &HolonReference) -> Result<bool, HolonError> {
    let expected = CoreHolonTypeName::TypeDescriptor.as_holon_name();
    match descriptor.property_value(CorePropertyTypeName::TypeName)? {
        Some(BaseValue::StringValue(type_name)) => Ok(type_name == expected),
        Some(BaseValue::EnumValue(type_name)) => Ok(type_name.0 == expected),
        Some(_) | None => Ok(false),
    }
}

/// Classifies whether a relationship type descriptor is declared or inverse.
///
/// The descriptor's effective `Extends` chain must reach either
/// [`CoreHolonTypeName::DeclaredRelationshipType`] or
/// [`CoreHolonTypeName::InverseRelationshipType`]; otherwise this returns
/// [`HolonError::WrongDescriptorKind`].
pub fn classify_relationship_direction(
    relationship_type_descriptor: &HolonReference,
) -> Result<RelationshipDirection, HolonError> {
    let declared_relationship_type = CoreHolonTypeName::DeclaredRelationshipType.as_holon_name();
    let inverse_relationship_type = CoreHolonTypeName::InverseRelationshipType.as_holon_name();
    let expected_relationship_type_names =
        [declared_relationship_type.clone(), inverse_relationship_type.clone()];

    search_extends_chain(
        relationship_type_descriptor,
        &expected_relationship_type_names,
        |type_name| {
            if type_name == &declared_relationship_type {
                return Some(RelationshipDirection::Declared);
            }

            if type_name == &inverse_relationship_type {
                return Some(RelationshipDirection::Inverse);
            }

            None
        },
    )
}

/// Collects related members across a descriptor's effective inheritance chain.
///
/// Members are returned in self-first ancestor order. A member reference that
/// appears more than once is included only at its first occurrence.
pub(crate) fn flatten_related_members(
    start: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    descriptor_semantics::flatten_related_members(&HolonReferenceGraph, start, &relationship_name)
        .map_err(map_semantics_error)
}

/// Collects inherited members and rejects distinct declarations with the same semantic name.
pub(crate) fn flatten_named_related_members(
    start: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
    declaration_kind: &'static str,
    semantic_name: impl FnMut(&HolonReference) -> Result<String, HolonError>,
) -> Result<Vec<HolonReference>, HolonError> {
    descriptor_semantics::flatten_named_members(
        &HolonReferenceGraph,
        start,
        &relationship_name,
        declaration_kind,
        semantic_name,
    )
    .map_err(map_semantics_error)
}

pub(crate) fn collect_named_related_members_from_lineage(
    lineage: impl IntoIterator<Item = HolonReference>,
    relationship_name: CoreRelationshipTypeName,
    declaration_kind: &'static str,
    semantic_name: impl FnMut(&HolonReference) -> Result<String, HolonError>,
) -> Result<Vec<HolonReference>, HolonError> {
    descriptor_semantics::collect_named_members_from_lineage(
        &HolonReferenceGraph,
        lineage,
        &relationship_name,
        declaration_kind,
        semantic_name,
    )
    .map_err(map_semantics_error)
}

pub(crate) fn effective_instance_key_rule(
    descriptor: &HolonReference,
) -> Result<Option<HolonReference>, HolonError> {
    descriptor_semantics::effective_instance_key_rule(
        &HolonReferenceGraph,
        descriptor,
        &CoreRelationshipTypeName::UsesKeyRule,
    )
    .map_err(map_semantics_error)
}

/// Lazy iterator over a descriptor's `Extends` lineage.
///
/// Traversal state and structural policy are owned by `descriptor_semantics`;
/// this wrapper preserves the established runtime iterator and `HolonError` API.
pub struct ExtendsIter {
    traversal: SemanticExtendsTraversal<HolonReference, String, HolonError>,
}

impl ExtendsIter {
    fn new(start: &HolonReference) -> Self {
        Self { traversal: SemanticExtendsTraversal::new(start.clone()) }
    }
}

impl Iterator for ExtendsIter {
    type Item = Result<HolonReference, HolonError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.traversal
            .next_with(HolonReference::reference_id_string, |current| {
                HolonReferenceGraph.extends_targets(current)
            })
            .map(|result| result.map_err(map_semantics_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};
    use crate::reference_layer::WritableHolon;

    fn declared_relationship_type_name() -> String {
        CoreHolonTypeName::DeclaredRelationshipType.as_holon_name().to_string()
    }

    fn inverse_relationship_type_name() -> String {
        CoreHolonTypeName::InverseRelationshipType.as_holon_name().to_string()
    }

    fn expected_relationship_kind() -> String {
        format!("{} or {}", declared_relationship_type_name(), inverse_relationship_type_name())
    }

    #[test]
    fn ancestors_returns_self_for_root_descriptor() -> Result<(), HolonError> {
        let context = build_context();
        let root = new_test_holon(&context, "root")?;
        let root_ref = HolonReference::from(&root);

        assert_eq!(ancestors(&root_ref)?, vec![root_ref]);

        Ok(())
    }

    #[test]
    fn ancestors_returns_linear_chain_in_self_first_order() -> Result<(), HolonError> {
        let context = build_context();
        let root = new_test_holon(&context, "a")?;
        let mut middle = new_test_holon(&context, "b")?;
        let mut leaf = new_test_holon(&context, "c")?;

        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.clone().into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.clone().into()])?;

        assert_eq!(
            ancestors(&HolonReference::from(&leaf))?,
            vec![
                HolonReference::from(&leaf),
                HolonReference::from(&middle),
                HolonReference::from(&root),
            ]
        );

        Ok(())
    }

    #[test]
    fn equals_or_extends_matches_self_and_ancestors_by_reference_identity() -> Result<(), HolonError>
    {
        let context = build_context();
        let root = new_test_holon(&context, "root")?;
        let unrelated = new_test_holon(&context, "root")?;
        let mut child = new_test_holon(&context, "child")?;

        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&root)],
        )?;

        let child_ref = HolonReference::from(&child);
        let root_ref = HolonReference::from(&root);
        let unrelated_ref = HolonReference::from(&unrelated);

        assert!(equals_or_extends(&child_ref, &child_ref)?);
        assert!(equals_or_extends(&child_ref, &root_ref)?);
        assert!(!equals_or_extends(&child_ref, &unrelated_ref)?);

        Ok(())
    }

    #[test]
    fn effective_descriptor_lineage_for_instance_uses_described_by_extends_chain(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let holon_type = new_descriptor_holon(&context, "HolonType", "HolonType", "Holon")?;
        let mut book_type = new_descriptor_holon(&context, "Book.HolonType", "Book", "Holon")?;
        let mut book = new_test_holon(&context, "book-instance")?;

        book_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&holon_type)],
        )?;
        book.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&book_type)],
        )?;

        assert_eq!(
            effective_descriptor_lineage(&HolonReference::from(&book))?,
            vec![HolonReference::from(&book_type), HolonReference::from(&holon_type)]
        );

        Ok(())
    }

    #[test]
    fn effective_descriptor_lineage_for_descriptor_combines_own_and_describing_lineages(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let meta_type_descriptor =
            new_descriptor_holon(&context, "MetaTypeDescriptor", "MetaTypeDescriptor", "Holon")?;
        let mut meta_holon_type =
            new_descriptor_holon(&context, "MetaHolonType", "MetaHolonType", "Holon")?;
        let mut holon_type = new_descriptor_holon(&context, "HolonType", "HolonType", "Holon")?;
        let mut type_descriptor =
            new_descriptor_holon(&context, "TypeDescriptor.HolonType", "TypeDescriptor", "Holon")?;
        let mut descriptor_holon = new_descriptor_holon(
            &context,
            "MetaTypeDescriptor.Instance",
            "MetaTypeDescriptor",
            "Holon",
        )?;

        meta_holon_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&meta_type_descriptor)],
        )?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&meta_holon_type)],
        )?;
        type_descriptor.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&holon_type)],
        )?;
        descriptor_holon.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&meta_type_descriptor)],
        )?;
        descriptor_holon.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&type_descriptor)],
        )?;

        assert_eq!(
            effective_descriptor_lineage(&HolonReference::from(&descriptor_holon))?,
            vec![
                HolonReference::from(&descriptor_holon),
                HolonReference::from(&meta_type_descriptor),
                HolonReference::from(&type_descriptor),
                HolonReference::from(&holon_type),
                HolonReference::from(&meta_holon_type),
            ]
        );

        Ok(())
    }

    #[test]
    fn classify_relationship_direction_returns_declared_for_declared_relationship_type(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let declared_relationship_type = new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &declared_relationship_type_name(),
            "Relationship",
        )?;
        let mut authored_books =
            new_descriptor_holon(&context, "authored-books", "AuthoredBooks", "Relationship")?;

        authored_books.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_relationship_type.into()],
        )?;

        assert_eq!(
            classify_relationship_direction(&HolonReference::from(&authored_books))?,
            RelationshipDirection::Declared
        );

        Ok(())
    }

    #[test]
    fn classify_relationship_direction_walks_multi_step_relationship_chain(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let declared_relationship_type = new_descriptor_holon(
            &context,
            "declared-relationship-type-multi-step",
            &declared_relationship_type_name(),
            "Relationship",
        )?;
        let mut intermediate_relationship_type = new_descriptor_holon(
            &context,
            "intermediate-relationship-type",
            "IntermediateRelationshipType",
            "Relationship",
        )?;
        intermediate_relationship_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_relationship_type.into()],
        )?;
        let mut authored_books = new_descriptor_holon(
            &context,
            "authored-books-multi-step",
            "AuthoredBooks",
            "Relationship",
        )?;

        authored_books.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![intermediate_relationship_type.into()],
        )?;

        assert_eq!(
            classify_relationship_direction(&HolonReference::from(&authored_books))?,
            RelationshipDirection::Declared
        );

        Ok(())
    }

    #[test]
    fn classify_relationship_direction_returns_inverse_for_inverse_relationship_type(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let inverse_relationship_type = new_descriptor_holon(
            &context,
            "inverse-relationship-type",
            &inverse_relationship_type_name(),
            "Relationship",
        )?;
        let mut authored_by =
            new_descriptor_holon(&context, "authored-by", "AuthoredBy", "Relationship")?;

        authored_by.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![inverse_relationship_type.into()],
        )?;

        assert_eq!(
            classify_relationship_direction(&HolonReference::from(&authored_by))?,
            RelationshipDirection::Inverse
        );

        Ok(())
    }

    #[test]
    fn classify_relationship_direction_errors_for_malformed_relationship_type(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let other_relationship_root = new_descriptor_holon(
            &context,
            "other-relationship-root",
            "OtherRelationshipRoot",
            "Relationship",
        )?;
        let mut malformed_relationship = new_descriptor_holon(
            &context,
            "malformed-relationship",
            "MalformedRelationship",
            "Relationship",
        )?;

        malformed_relationship.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![other_relationship_root.into()],
        )?;

        assert!(matches!(
            classify_relationship_direction(&HolonReference::from(&malformed_relationship)),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == expected_relationship_kind()
                    && found == "MalformedRelationship"
        ));

        Ok(())
    }

    #[test]
    fn ancestors_errors_when_descriptor_has_multiple_extends() -> Result<(), HolonError> {
        let context = build_context();
        let parent_a = new_test_holon(&context, "parent-a")?;
        let parent_b = new_test_holon(&context, "parent-b")?;
        let mut child = new_test_holon(&context, "child")?;

        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;

        assert!(matches!(
            ancestors(&HolonReference::from(&child)),
            Err(HolonError::MultipleExtends { count, .. }) if count == 2
        ));

        Ok(())
    }

    #[test]
    fn ancestors_errors_on_extends_cycles_and_self_loops() -> Result<(), HolonError> {
        let context = build_context();
        let mut a = new_test_holon(&context, "cycle-a")?;
        let mut b = new_test_holon(&context, "cycle-b")?;

        a.add_related_holons(CoreRelationshipTypeName::Extends, vec![b.clone().into()])?;
        b.add_related_holons(CoreRelationshipTypeName::Extends, vec![a.clone().into()])?;

        assert!(matches!(
            ancestors(&HolonReference::from(&a)),
            Err(HolonError::CyclicExtends { .. })
        ));

        let mut self_loop = new_test_holon(&context, "self-loop")?;
        self_loop.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![self_loop.clone().into()],
        )?;

        assert!(matches!(
            ancestors(&HolonReference::from(&self_loop)),
            Err(HolonError::CyclicExtends { .. })
        ));

        Ok(())
    }

    #[test]
    fn walk_extends_chain_short_circuits_before_errors_further_up_chain() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut ancestor = new_test_holon(&context, "ancestor")?;
        let mut middle = new_test_holon(&context, "middle")?;
        let mut leaf = new_test_holon(&context, "leaf")?;

        ancestor
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![ancestor.clone().into()])?;
        middle
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![ancestor.clone().into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.clone().into()])?;

        let first_two = walk_extends_chain(&HolonReference::from(&leaf))
            .take(2)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(first_two, vec![HolonReference::from(&leaf), HolonReference::from(&middle)]);

        Ok(())
    }

    #[test]
    fn flatten_related_members_preserves_lineage_order_and_deduplicates() -> Result<(), HolonError>
    {
        let context = build_context();
        let member_a = new_test_holon(&context, "member-a")?;
        let member_b = new_test_holon(&context, "member-b")?;
        let member_c = new_test_holon(&context, "member-c")?;
        let mut root = new_test_holon(&context, "root")?;
        let mut middle = new_test_holon(&context, "middle")?;
        let mut leaf = new_test_holon(&context, "leaf")?;

        root.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![member_a.clone().into()],
        )?;
        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.clone().into()])?;
        middle.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![member_b.clone().into(), member_a.clone().into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.clone().into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![member_c.clone().into(), member_b.clone().into()],
        )?;

        assert_eq!(
            flatten_related_members(
                &HolonReference::from(&leaf),
                CoreRelationshipTypeName::InstanceProperties,
            )?,
            vec![
                HolonReference::from(&member_c),
                HolonReference::from(&member_b),
                HolonReference::from(&member_a),
            ]
        );

        Ok(())
    }
}
