//! Staged-safe classification of authored relationship occurrences.
//!
//! A source contract is the effective declared relationship surface of one
//! source holon. It is intentionally distinct from available read navigation:
//! the latter may include materialized inverse relationships, while authoring
//! is licensed only by declared relationships. When a requested name is not a
//! declared source relationship, classification inspects the concrete target
//! endpoint to recognize an inverse occurrence without relying on `TargetOf`.

use crate::descriptors::{
    effective_relationships::effective_declared_relationships_for_holon, equals_or_extends,
    DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor, InverseRelationshipDescriptor,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, RelationshipName};

/// Orientation of one populated `(source, relationship name, target)` occurrence.
///
/// `Unresolved` means that descriptor lookup completed but neither a declared
/// source contract nor an applicable opposite-endpoint inverse established the
/// orientation. It is not an authorization result.
pub enum RelationshipOccurrenceOrientation {
    /// The source contract declares this relationship name.
    Declared(DeclaredRelationshipDescriptor),
    /// The occurrence names an inverse of a declared relationship licensed on
    /// the concrete target endpoint.
    RecognizedInverse {
        inverse: InverseRelationshipDescriptor,
        declared: DeclaredRelationshipDescriptor,
        declared_source_type: HolonDescriptor,
    },
    /// Neither declared nor recognized inverse after reliable lookup.
    Unresolved,
}

/// Effective declared relationship contract for a concrete source holon.
///
/// This value is assessment-local. It performs no persistent caching and can
/// be constructed for staged schema and instance graphs because it relies only
/// on the `InstanceRelationships` inheritance surface.
pub struct SourceRelationshipContract {
    source: HolonReference,
    source_descriptor: HolonDescriptor,
    declared_relationships: Vec<DeclaredRelationshipDescriptor>,
}

impl SourceRelationshipContract {
    /// Resolves the source's effective declared relationship contract once.
    pub fn resolve(source: HolonReference) -> Result<Self, HolonError> {
        let source_descriptor = source.holon_descriptor()?;
        let declared_relationships = effective_declared_relationships_for_holon(&source)?;
        Ok(Self { source, source_descriptor, declared_relationships })
    }

    /// Resolves `relationship_name` as a declared relationship authored by the
    /// source, if present.
    pub fn declared_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Option<&DeclaredRelationshipDescriptor>, HolonError> {
        for declared in &self.declared_relationships {
            if declared.base_relationship_name()? == *relationship_name {
                return Ok(Some(declared));
            }
        }
        Ok(None)
    }

    /// Classifies one populated relationship occurrence against this source
    /// and its concrete target endpoint.
    ///
    /// The declared source contract wins without traversing the target. If it
    /// does not contain the requested name, the target's effective declared
    /// contract is inspected for an inverse whose target type admits this
    /// source descriptor. Multiple distinct matches are ambiguous and fail
    /// rather than selecting an arbitrary descriptor.
    pub fn classify_occurrence(
        &self,
        relationship_name: &RelationshipName,
        target: &HolonReference,
    ) -> Result<RelationshipOccurrenceOrientation, HolonError> {
        if let Some(declared) = self.declared_relationship(relationship_name)? {
            return Ok(RelationshipOccurrenceOrientation::Declared(
                DeclaredRelationshipDescriptor::try_from_holon(declared.holon().clone())?,
            ));
        }

        let target_descriptor = match target.holon_descriptor() {
            Ok(descriptor) => descriptor,
            // Incomplete assembly may not yet have bound the target. The
            // caller still retains the original undeclared-name rejection;
            // lack of an endpoint contract simply cannot recognize an inverse
            // occurrence early.
            Err(HolonError::MissingDescribedBy { .. })
            | Err(HolonError::MultipleDescribedBy { .. }) => {
                return Ok(RelationshipOccurrenceOrientation::Unresolved)
            }
            Err(error) => return Err(error),
        };
        let mut inverse_match: Option<(
            InverseRelationshipDescriptor,
            DeclaredRelationshipDescriptor,
            HolonDescriptor,
        )> = None;
        for declared in target_descriptor.effective_declared_relationships()? {
            let inverse = declared.required_inverse()?;
            if inverse.base_relationship_name()? != *relationship_name
                || !equals_or_extends(
                    self.source_descriptor.holon(),
                    declared.target_type()?.holon(),
                )?
            {
                continue;
            }

            if let Some((_, existing_declared, _)) = &inverse_match {
                if existing_declared.holon().reference_id_string()
                    != declared.holon().reference_id_string()
                {
                    return Err(HolonError::AmbiguousRelationshipTraversal {
                        relationship: relationship_name.to_string(),
                        descriptor: self.source.reference_id_string(),
                    });
                }
                continue;
            }

            let declared_source_type = declared.source_type()?;
            inverse_match = Some((inverse, declared, declared_source_type));
        }

        Ok(match inverse_match {
            Some((inverse, declared, declared_source_type)) => {
                RelationshipOccurrenceOrientation::RecognizedInverse {
                    inverse,
                    declared,
                    declared_source_type,
                }
            }
            None => RelationshipOccurrenceOrientation::Unresolved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use base_types::MapString;
    use core_types::RelationshipName;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    struct Fixture {
        book: TransientReference,
        person: TransientReference,
    }

    fn relationship_name(name: &str) -> RelationshipName {
        RelationshipName(MapString(name.to_owned()))
    }

    fn fixture() -> Result<Fixture, HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "occurrence-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "occurrence-inverse-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let book_type = new_holon_type_descriptor(&context, "occurrence-book-type", "Book")?;
        let mut person_type =
            new_holon_type_descriptor(&context, "occurrence-person-type", "Person")?;
        let mut written_by = new_relationship_descriptor_holon(
            &context,
            "occurrence-written-by",
            "WrittenBy",
            HolonReference::from(&person_type),
            HolonReference::from(&book_type),
        )?;
        let mut authors = new_relationship_descriptor_holon(
            &context,
            "occurrence-authors",
            "Authors",
            HolonReference::from(&book_type),
            HolonReference::from(&person_type),
        )?;
        written_by.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&declared_type)],
        )?;
        authors.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&inverse_type)],
        )?;
        written_by.add_related_holons(
            CoreRelationshipTypeName::HasInverse,
            vec![HolonReference::from(&authors)],
        )?;
        authors.add_related_holons(
            CoreRelationshipTypeName::InverseOf,
            vec![HolonReference::from(&written_by)],
        )?;
        person_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![HolonReference::from(&written_by)],
        )?;

        let mut book = new_test_holon(&context, "occurrence-book")?;
        let mut person = new_test_holon(&context, "occurrence-person")?;
        book.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&book_type)],
        )?;
        person.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&person_type)],
        )?;

        Ok(Fixture { book, person })
    }

    #[test]
    fn classifies_declared_inverse_and_unresolved_without_target_of() -> Result<(), HolonError> {
        let fixture = fixture()?;

        let person_contract = SourceRelationshipContract::resolve((&fixture.person).into())?;
        assert!(matches!(
            person_contract.classify_occurrence(
                &relationship_name("WrittenBy"),
                &HolonReference::from(&fixture.book),
            )?,
            RelationshipOccurrenceOrientation::Declared(_)
        ));

        let book_contract = SourceRelationshipContract::resolve((&fixture.book).into())?;
        assert!(matches!(
            book_contract.classify_occurrence(
                &relationship_name("Authors"),
                &HolonReference::from(&fixture.person),
            )?,
            RelationshipOccurrenceOrientation::RecognizedInverse { .. }
        ));
        assert!(matches!(
            book_contract.classify_occurrence(
                &relationship_name("Unresolved"),
                &HolonReference::from(&fixture.person),
            )?,
            RelationshipOccurrenceOrientation::Unresolved
        ));

        Ok(())
    }
}
