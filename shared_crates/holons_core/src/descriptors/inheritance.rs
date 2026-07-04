use std::collections::HashSet;

use crate::descriptors::accessor_helpers::{descriptor_label, lock_error, search_extends_chain};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

/// Direction of a relationship type descriptor relative to its declared edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipDirection {
    /// The descriptor names the canonical source-to-target relationship.
    Declared,
    /// The descriptor names the inverse target-to-source relationship.
    Inverse,
}

/// Resolves the direct `Extends` parent for a descriptor holon.
///
/// Cardinality is enforced here so all iterator-based callers inherit the same
/// multiple-parent error semantics.
pub(crate) fn extends_parent(holon: &HolonReference) -> Result<Option<HolonReference>, HolonError> {
    let collection_arc = holon.related_holons(CoreRelationshipTypeName::Extends)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    let members = collection.get_members();

    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
        many => Err(HolonError::MultipleExtends {
            descriptor: descriptor_label(holon),
            count: many.len(),
        }),
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
    let anchor_id = anchor.reference_id_string();

    for ancestor in walk_extends_chain(candidate) {
        if ancestor?.reference_id_string() == anchor_id {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Computes the effective descriptor lineage for a holon(per MAP Type System) v1.2.
///
/// Ordinary instances use the `Extends` lineage of their `DescribedBy` descriptor.
/// Descriptor holons additionally contribute their own `Extends` lineage before
/// the effective lineage of their describing `TypeDescriptor` descriptor.
pub fn effective_descriptor_lineage(
    holon: &HolonReference,
) -> Result<Vec<HolonReference>, HolonError> {
    let Some(described_by_descriptor) = described_by_descriptor(holon)? else {
        return ancestors(holon);
    };

    if is_type_descriptor_descriptor(&described_by_descriptor)? {
        let mut lineage = ancestors(holon)?;
        append_unique(&mut lineage, ancestors(&described_by_descriptor)?);
        return Ok(lineage);
    }

    ancestors(&described_by_descriptor)
}

pub(crate) fn described_by_descriptor(
    holon: &HolonReference,
) -> Result<Option<HolonReference>, HolonError> {
    let collection_arc = holon.related_holons(CoreRelationshipTypeName::DescribedBy)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    let members = collection.get_members();

    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
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

fn append_unique(lineage: &mut Vec<HolonReference>, additional: Vec<HolonReference>) {
    let mut seen = lineage.iter().map(HolonReference::reference_id_string).collect::<HashSet<_>>();

    for descriptor in additional {
        if seen.insert(descriptor.reference_id_string()) {
            lineage.push(descriptor);
        }
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
    let mut members = Vec::new();
    let mut seen = HashSet::new();

    for ancestor in walk_extends_chain(start) {
        let ancestor = ancestor?;
        let collection_arc = ancestor.related_holons(relationship_name.clone())?;
        let collection = collection_arc.read().map_err(lock_error)?;

        for member in collection.get_members() {
            if seen.insert(member.reference_id_string()) {
                members.push(member.clone());
            }
        }
    }

    Ok(members)
}

/// Lazy iterator over a descriptor's `Extends` lineage.
///
/// State model:
/// - `next` is the next descriptor to yield
/// - `pending_error` stores a structural error discovered while preparing the
///   next step
/// - `visited` tracks already-yielded descriptors for cycle detection
/// - `finished` marks terminal exhaustion after either the root or an error
pub struct ExtendsIter {
    next: Option<HolonReference>,
    pending_error: Option<HolonError>,
    visited: HashSet<String>,
    finished: bool,
}

impl ExtendsIter {
    fn new(start: &HolonReference) -> Self {
        Self {
            next: Some(start.clone()),
            pending_error: None,
            visited: HashSet::new(),
            finished: false,
        }
    }
}

impl Iterator for ExtendsIter {
    type Item = Result<HolonReference, HolonError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Terminal state: once the chain is exhausted or an error has been
        // emitted, iteration stays closed.
        if self.finished {
            return None;
        }

        // Deferred error emission preserves the current item for callers that
        // intentionally stop early (for example, first-match inheritance lookups).
        if let Some(error) = self.pending_error.take() {
            self.finished = true;
            return Some(Err(error));
        }

        let current = self.next.take()?;
        // Cycle detection relies on reference_id_string() being a stable,
        // collision-resistant identity for each concrete holon reference. Do
        // not implement reference_id_string() with lossy display fallbacks such
        // as "<invalid utf-8>" for binary saved IDs.
        self.visited.insert(current.reference_id_string());

        // Resolve the next step after capturing the current item. This keeps
        // the iterator self-first while still surfacing cycles and
        // multiple-parent structures on the following call.
        match extends_parent(&current) {
            Ok(Some(parent)) => {
                if self.visited.contains(&parent.reference_id_string()) {
                    self.pending_error =
                        Some(HolonError::CyclicExtends { descriptor: descriptor_label(&parent) });
                } else {
                    self.next = Some(parent);
                }
            }
            Ok(None) => {
                self.finished = true;
            }
            Err(error) => {
                self.pending_error = Some(error);
            }
        }

        Some(Ok(current))
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
