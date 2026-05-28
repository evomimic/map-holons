use crate::descriptors::{
    accessor_helpers, DeclaredRelationshipDescriptor, Descriptor, InverseRelationshipDescriptor,
    TypeHeader,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use crate::StagedReference;
use core_types::{HolonError, RelationshipName};
use type_names::CoreRelationshipTypeName;

/// Resolves the inverse relationship name for a declared relationship on `source_ref`.
///
/// Normal persisted schema resolution uses the declared descriptor's materialized
/// `HasInverse` edge. During bootstrap, when `HasInverse` may be the inverse
/// SmartLink currently being established, this falls back to staged inverse
/// relationship descriptors whose `InverseOf` points at the declared descriptor.
pub fn resolve_inverse_relationship_name(
    source_ref: &HolonReference,
    forward_name: &RelationshipName,
    staged_references: &[StagedReference],
) -> Result<RelationshipName, HolonError> {
    // Resolve the declared descriptor through the source holon's schema.
    let source_descriptor = source_ref.holon_descriptor()?;
    let declared_descriptor = source_descriptor
        .get_relationship_by_name(forward_name.clone())?
        .try_into_declared_relationship_descriptor()?;

    // Prefer the normal materialized HasInverse path.
    if let Some(inverse_descriptor) = declared_descriptor.has_inverse()? {
        return Ok(RelationshipName(inverse_descriptor.header().type_name()?));
    }

    // Bootstrap fallback: derive the inverse from staged InverseOf declarations.
    let matching_inverse_descriptors =
        staged_inverse_descriptors_for_declared(&declared_descriptor, staged_references)?;

    match matching_inverse_descriptors.as_slice() {
        // Commit is ultimately trying to materialize HasInverse. A missing or
        // ambiguous bootstrap fallback means that required local schema contract
        // still cannot be upheld.
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: CoreRelationshipTypeName::HasInverse.as_relationship_name().to_string(),
            descriptor: accessor_helpers::descriptor_label(declared_descriptor.holon()),
        }),
        [inverse_descriptor] => Ok(RelationshipName(inverse_descriptor.header().type_name()?)),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: CoreRelationshipTypeName::HasInverse.as_relationship_name().to_string(),
            descriptor: accessor_helpers::descriptor_label(declared_descriptor.holon()),
            count: many.len(),
        }),
    }
}

fn staged_inverse_descriptors_for_declared(
    declared_descriptor: &DeclaredRelationshipDescriptor,
    staged_references: &[StagedReference],
) -> Result<Vec<InverseRelationshipDescriptor>, HolonError> {
    let declared_reference_id = declared_descriptor.holon().reference_id_string();
    let mut matching_inverse_descriptors = Vec::new();

    for staged_reference in staged_references {
        // Only concrete relationship descriptors can be authored inverse declarations.
        // Abstract roots like `InverseRelationshipType` narrow through their own
        // type name, but they intentionally do not carry an `InverseOf` edge.
        let candidate_reference = HolonReference::from(staged_reference);
        if !is_concrete_relationship_descriptor(&candidate_reference) {
            continue;
        }

        let Ok(candidate_descriptor) =
            InverseRelationshipDescriptor::try_from_holon(candidate_reference)
        else {
            continue;
        };

        let inverse_of = candidate_descriptor.inverse_of()?;
        // Match the candidate's InverseOf target to the declared descriptor by
        // stable staged reference identity, not by display text.
        if inverse_of.holon().reference_id_string() == declared_reference_id {
            matching_inverse_descriptors.push(candidate_descriptor);
        }
    }

    Ok(matching_inverse_descriptors)
}

fn is_concrete_relationship_descriptor(candidate_reference: &HolonReference) -> bool {
    let header = TypeHeader::new(candidate_reference);

    // The bootstrap closure includes descriptors, abstract descriptor roots, and
    // ordinary holons. Only concrete relationship descriptors can declare an inverse.
    let Ok(instance_type_kind) = header.instance_type_kind() else {
        return false;
    };
    if instance_type_kind.0 != "Relationship" {
        return false;
    }

    matches!(header.is_abstract_type(), Ok(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::{StagedReference, TransientReference, WritableHolon};
    use base_types::MapString;
    use std::sync::Arc;
    use type_names::CoreHolonTypeName;

    struct RelationshipSchemaFixture {
        context: Arc<TransactionContext>,
        source: TransientReference,
        source_type: StagedReference,
        declared: StagedReference,
        inverse: StagedReference,
    }

    impl RelationshipSchemaFixture {
        fn staged_references(&self) -> Vec<StagedReference> {
            vec![self.declared.clone(), self.inverse.clone()]
        }
    }

    fn build_relationship_schema(
        relationship_name: &str,
        inverse_name: &str,
        materialize_has_inverse: bool,
    ) -> Result<RelationshipSchemaFixture, HolonError> {
        let context = build_context();

        let declared_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?)?;
        let inverse_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            "inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let source_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "book-type",
            "BookType",
        )?)?;
        let target_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "person-type",
            "PersonType",
        )?)?;

        let declared_transient = new_relationship_descriptor_holon(
            &context,
            "declared-relationship",
            relationship_name,
            (&source_type).into(),
            (&target_type).into(),
        )?;
        let inverse_transient = new_relationship_descriptor_holon(
            &context,
            "inverse-relationship",
            inverse_name,
            (&target_type).into(),
            (&source_type).into(),
        )?;

        let mut declared = context.mutation().stage_new_holon(declared_transient)?;
        let mut inverse = context.mutation().stage_new_holon(inverse_transient)?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&declared_type).into()])?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&inverse_type).into()])?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::InverseOf, vec![(&declared).into()])?;
        if materialize_has_inverse {
            declared.add_related_holons(
                CoreRelationshipTypeName::HasInverse,
                vec![(&inverse).into()],
            )?;
        }

        let mut source_type = source_type;
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;

        let mut source = new_test_holon(&context, "book-instance")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![(&source_type).into()],
        )?;

        Ok(RelationshipSchemaFixture { context, source, source_type, declared, inverse })
    }

    fn authored_by() -> RelationshipName {
        RelationshipName(MapString("AuthoredBy".to_string()))
    }

    #[test]
    fn resolves_materialized_has_inverse_relationship() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", true)?;

        let inverse_name =
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by(), &[])?;

        assert_eq!(inverse_name, RelationshipName(MapString("Authors".to_string())));
        Ok(())
    }

    #[test]
    fn materialized_has_inverse_takes_precedence_over_staged_fallback() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", true)?;
        let inverse_type = fixture.context.mutation().stage_new_holon(new_descriptor_holon(
            &fixture.context,
            "fallback-inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let target_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "fallback-person-type",
            "PersonType",
        )?)?;
        let source_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "fallback-book-type",
            "BookType",
        )?)?;
        let fallback_inverse_transient = new_relationship_descriptor_holon(
            &fixture.context,
            "fallback-inverse-relationship",
            "FallbackAuthors",
            source_type.into(),
            target_type.into(),
        )?;
        let mut fallback_inverse =
            fixture.context.mutation().stage_new_holon(fallback_inverse_transient)?;
        fallback_inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        fallback_inverse.add_related_holons(
            CoreRelationshipTypeName::InverseOf,
            vec![(&fixture.declared).into()],
        )?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&fixture.source).into(),
            &authored_by(),
            &[fallback_inverse],
        )?;

        assert_eq!(inverse_name, RelationshipName(MapString("Authors".to_string())));
        Ok(())
    }

    #[test]
    fn resolves_bootstrap_inverse_from_staged_inverse_of() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&fixture.source).into(),
            &authored_by(),
            &fixture.staged_references(),
        )?;

        assert_eq!(inverse_name, RelationshipName(MapString("Authors".to_string())));
        Ok(())
    }

    #[test]
    fn bootstrap_fallback_skips_abstract_inverse_relationship_type_root() -> Result<(), HolonError>
    {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;
        let mut abstract_inverse_type =
            new_test_holon(&fixture.context, "abstract-inverse-relationship-type")?;
        abstract_inverse_type
            .with_property_value(
                type_names::CorePropertyTypeName::TypeName,
                core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            )?
            .with_property_value(type_names::CorePropertyTypeName::IsAbstractType, true)?
            .with_property_value(type_names::CorePropertyTypeName::InstanceTypeKind, "Holon")?;
        let abstract_inverse_type =
            fixture.context.mutation().stage_new_holon(abstract_inverse_type)?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&fixture.source).into(),
            &authored_by(),
            &[abstract_inverse_type, fixture.inverse.clone()],
        )?;

        assert_eq!(inverse_name, RelationshipName(MapString("Authors".to_string())));
        Ok(())
    }

    #[test]
    fn concrete_inverse_candidate_missing_inverse_of_errors() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;
        let inverse_type = fixture.context.mutation().stage_new_holon(new_descriptor_holon(
            &fixture.context,
            "broken-inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Holon",
        )?)?;
        let target_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "broken-person-type",
            "PersonType",
        )?)?;
        let source_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "broken-book-type",
            "BookType",
        )?)?;
        let broken_inverse_transient = new_relationship_descriptor_holon(
            &fixture.context,
            "broken-inverse-relationship",
            "BrokenAuthors",
            source_type.into(),
            target_type.into(),
        )?;
        let mut broken_inverse =
            fixture.context.mutation().stage_new_holon(broken_inverse_transient)?;
        broken_inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;

        assert!(matches!(
            resolve_inverse_relationship_name(
                &(&fixture.source).into(),
                &authored_by(),
                &[broken_inverse, fixture.inverse.clone()],
            ),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "InverseOf"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_inverse_cannot_be_resolved() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by(), &[]),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "HasInverse"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_bootstrap_inverse_resolution_is_ambiguous() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;
        let inverse_type = fixture.context.mutation().stage_new_holon(new_descriptor_holon(
            &fixture.context,
            "second-inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let target_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "second-person-type",
            "PersonType",
        )?)?;
        let source_type = fixture.context.mutation().stage_new_holon(new_holon_type_descriptor(
            &fixture.context,
            "second-book-type",
            "BookType",
        )?)?;
        let second_inverse_transient = new_relationship_descriptor_holon(
            &fixture.context,
            "second-inverse-relationship",
            "AuthoredWorks",
            source_type.into(),
            target_type.into(),
        )?;
        let mut second_inverse =
            fixture.context.mutation().stage_new_holon(second_inverse_transient)?;
        second_inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        second_inverse.add_related_holons(
            CoreRelationshipTypeName::InverseOf,
            vec![(&fixture.declared).into()],
        )?;

        let mut staged_references = fixture.staged_references();
        staged_references.push(second_inverse);

        assert!(matches!(
            resolve_inverse_relationship_name(
                &(&fixture.source).into(),
                &authored_by(),
                &staged_references,
            ),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "HasInverse" && count == 2
        ));
        Ok(())
    }

    #[test]
    fn errors_when_source_is_undescribed() -> Result<(), HolonError> {
        let context = build_context();
        let source = new_test_holon(&context, "undescribed-source")?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&source).into(), &authored_by(), &[]),
            Err(HolonError::MissingDescribedBy { .. })
        ));
        Ok(())
    }

    #[test]
    fn errors_when_relationship_is_not_declared_on_source_descriptor() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("PublishedBy", "Publishes", true)?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by(), &[]),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_relationship_descriptor_is_not_declared_kind() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("PublishedBy", "Publishes", true)?;
        let inverse_type = fixture.context.mutation().stage_new_holon(new_descriptor_holon(
            &fixture.context,
            "wrong-kind-inverse-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let mut wrong_kind_relationship =
            fixture.context.mutation().stage_new_holon(new_descriptor_holon(
                &fixture.context,
                "wrong-kind-authored-by",
                "AuthoredBy",
                "Relationship",
            )?)?;
        wrong_kind_relationship
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        let mut source_type = fixture.source_type.clone();
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&wrong_kind_relationship).into()],
        )?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by(), &[]),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType)
                    && found == "AuthoredBy"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_has_inverse_target_is_not_inverse_kind() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors", false)?;
        let mut declared = fixture.declared.clone();
        declared.add_related_holons(
            CoreRelationshipTypeName::HasInverse,
            vec![(&fixture.declared).into()],
        )?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by(), &[]),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::InverseRelationshipType)
                    && found == "AuthoredBy"
        ));
        Ok(())
    }
}
