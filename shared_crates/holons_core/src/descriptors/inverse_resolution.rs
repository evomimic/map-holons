use crate::descriptors::{accessor_helpers, effective_relationship_declaration, Descriptor};
use crate::reference_layer::HolonReference;
use core_types::{HolonError, RelationshipName};
use type_names::CoreRelationshipTypeName;

/// Resolves the inverse relationship name for a declared relationship on `source_ref`.
///
/// The declared descriptor must carry exactly one `HasInverse` target; commit uses
/// that declared-side edge to materialize the reciprocal SmartLink.
pub fn resolve_inverse_relationship_name(
    source_ref: &HolonReference,
    forward_name: &RelationshipName,
) -> Result<RelationshipName, HolonError> {
    // Resolve the declared descriptor through the source holon's effective surface.
    let declared_descriptor = effective_relationship_declaration(source_ref, forward_name)?
        .try_into_declared_relationship_descriptor()?;

    if let Some(inverse_descriptor) = declared_descriptor.has_inverse()? {
        return Ok(RelationshipName(inverse_descriptor.header().type_name()?));
    }

    Err(HolonError::MissingRequiredRelationship {
        relationship: CoreRelationshipTypeName::HasInverse.as_relationship_name().to_string(),
        descriptor: accessor_helpers::descriptor_label(declared_descriptor.holon()),
    })
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
    }

    fn build_relationship_schema(
        relationship_name: &str,
        inverse_name: &str,
    ) -> Result<RelationshipSchemaFixture, HolonError> {
        build_relationship_schema_with_has_inverse(relationship_name, inverse_name, true)
    }

    fn build_relationship_schema_with_has_inverse(
        relationship_name: &str,
        inverse_name: &str,
        author_has_inverse: bool,
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
        if author_has_inverse {
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

        Ok(RelationshipSchemaFixture { context, source, source_type, declared })
    }

    fn authored_by() -> RelationshipName {
        RelationshipName(MapString("AuthoredBy".to_string()))
    }

    #[test]
    fn resolves_materialized_has_inverse_relationship() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("AuthoredBy", "Authors")?;

        let inverse_name =
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by())?;

        assert_eq!(inverse_name, RelationshipName(MapString("Authors".to_string())));
        Ok(())
    }

    #[test]
    fn errors_when_inverse_cannot_be_resolved() -> Result<(), HolonError> {
        let fixture = build_relationship_schema_with_has_inverse("AuthoredBy", "Authors", false)?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by()),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "HasInverse"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_source_is_undescribed() -> Result<(), HolonError> {
        let context = build_context();
        let source = new_test_holon(&context, "undescribed-source")?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&source).into(), &authored_by()),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));
        Ok(())
    }

    #[test]
    fn resolves_descriptor_holon_relationship_through_own_extends_lineage() -> Result<(), HolonError>
    {
        let context = build_context();
        let type_descriptor = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "type-descriptor",
            "TypeDescriptor",
        )?)?;
        let meta_relationship_type = context.mutation().stage_new_holon(
            new_holon_type_descriptor(&context, "meta-relationship-type", "MetaRelationshipType")?,
        )?;
        let mut declared_relationship_type =
            context.mutation().stage_new_holon(new_descriptor_holon(
                &context,
                "declared-relationship-type-for-source-type",
                &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
                "Relationship",
            )?)?;
        let inverse_relationship_type =
            context.mutation().stage_new_holon(new_descriptor_holon(
                &context,
                "inverse-relationship-type-for-source-type",
                &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
                "Relationship",
            )?)?;

        let source_type_transient = new_relationship_descriptor_holon(
            &context,
            "source-type",
            "SourceType",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        let source_for_transient = new_relationship_descriptor_holon(
            &context,
            "source-for",
            "SourceFor",
            (&type_descriptor).into(),
            (&meta_relationship_type).into(),
        )?;
        let mut source_type = context.mutation().stage_new_holon(source_type_transient)?;
        let mut source_for = context.mutation().stage_new_holon(source_for_transient)?;

        declared_relationship_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&meta_relationship_type).into()],
        )?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_relationship_type.into()],
        )?;
        source_type
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&source_for).into()])?;
        source_for.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![inverse_relationship_type.into()],
        )?;

        let mut meta_relationship_type = meta_relationship_type;
        meta_relationship_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&source_type).into()],
        )?;

        let mut concrete_relationship =
            context.mutation().stage_new_holon(new_relationship_descriptor_holon(
                &context,
                "affords-operator",
                "AffordsOperator",
                (&meta_relationship_type).into(),
                (&type_descriptor).into(),
            )?)?;
        concrete_relationship
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&source_type).into()])?;
        concrete_relationship.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.into()],
        )?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&concrete_relationship).into(),
            &RelationshipName(MapString("SourceType".to_string())),
        )?;

        assert_eq!(inverse_name, RelationshipName(MapString("SourceFor".to_string())));
        Ok(())
    }

    #[test]
    fn errors_when_relationship_is_not_declared_on_source_descriptor() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("PublishedBy", "Publishes")?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by()),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_relationship_descriptor_is_not_declared_kind() -> Result<(), HolonError> {
        let fixture = build_relationship_schema("PublishedBy", "Publishes")?;
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
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType)
                    && found == "AuthoredBy"
        ));
        Ok(())
    }

    #[test]
    fn errors_when_has_inverse_target_is_not_inverse_kind() -> Result<(), HolonError> {
        let fixture = build_relationship_schema_with_has_inverse("AuthoredBy", "Authors", false)?;
        let mut declared = fixture.declared.clone();
        declared.add_related_holons(
            CoreRelationshipTypeName::HasInverse,
            vec![(&fixture.declared).into()],
        )?;

        assert!(matches!(
            resolve_inverse_relationship_name(&(&fixture.source).into(), &authored_by()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::InverseRelationshipType)
                    && found == "AuthoredBy"
        ));
        Ok(())
    }
}
