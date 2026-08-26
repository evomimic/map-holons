use crate::descriptors::effective_relationship_declaration;
use crate::reference_layer::HolonReference;
use core_types::{HolonError, RelationshipName};
use type_names::CoreRelationshipTypeName;

/// Resolves the inverse relationship name for a declared relationship on `source_ref`.
///
/// The declared descriptor must carry exactly one `HasInverse` target; commit uses
/// that declared-side edge to materialize the reciprocal SmartLink.
///
/// `OwnedBy` is the one exception. Space membership is infrastructure-supplied — staging
/// stamps it on every new lineage, before the holon has been given a `DescribedBy` and,
/// during the first core-schema load, before the `OwnedBy` descriptor itself has been
/// persisted. Routing it through the descriptor surface would therefore make ownership
/// depend on schema load order and would break commit for undescribed holons, so its
/// inverse is resolved structurally instead.
pub fn resolve_inverse_relationship_name(
    source_ref: &HolonReference,
    forward_name: &RelationshipName,
) -> Result<RelationshipName, HolonError> {
    if *forward_name == CoreRelationshipTypeName::OwnedBy.as_relationship_name() {
        return Ok(CoreRelationshipTypeName::Owns.as_relationship_name());
    }

    // Resolve the declared descriptor through the source holon's effective surface.
    let declared_descriptor = effective_relationship_declaration(source_ref, forward_name)?
        .try_into_declared_relationship_descriptor()?;

    let inverse_descriptor = declared_descriptor.required_inverse()?;
    Ok(RelationshipName(inverse_descriptor.header().type_name()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use base_types::MapString;
    use std::sync::Arc;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    struct RelationshipSchemaFixture {
        context: Arc<TransactionContext>,
        source: TransientReference,
        source_type: TransientReference,
        declared: TransientReference,
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

        let declared_type = new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut source_type = new_holon_type_descriptor(&context, "book-type", "BookType")?;
        let target_type = new_holon_type_descriptor(&context, "person-type", "PersonType")?;

        let mut declared = new_relationship_descriptor_holon(
            &context,
            "declared-relationship",
            relationship_name,
            (&source_type).into(),
            (&target_type).into(),
        )?;
        let mut inverse = new_relationship_descriptor_holon(
            &context,
            "inverse-relationship",
            inverse_name,
            (&target_type).into(),
            (&source_type).into(),
        )?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        if author_has_inverse {
            declared.add_related_holons(
                CoreRelationshipTypeName::HasInverse,
                vec![(&inverse).into()],
            )?;
        }

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
            Err(HolonError::MissingDescribedBy { .. })
        ));
        Ok(())
    }

    #[test]
    fn owned_by_resolves_structurally_without_a_descriptor() -> Result<(), HolonError> {
        // Space membership is stamped at staging time, before `DescribedBy` is attached and
        // (on the first core-schema load) before the `OwnedBy` descriptor exists at all.
        let context = build_context();
        let source = new_test_holon(&context, "undescribed-source")?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&source).into(),
            &CoreRelationshipTypeName::OwnedBy.as_relationship_name(),
        )?;

        assert_eq!(inverse_name, CoreRelationshipTypeName::Owns.as_relationship_name());
        Ok(())
    }

    #[test]
    fn descriptor_own_extends_lineage_does_not_license_inverse_resolution() -> Result<(), HolonError>
    {
        let context = build_context();
        let type_descriptor =
            new_holon_type_descriptor(&context, "type-descriptor", "TypeDescriptor")?;
        let mut meta_relationship_type =
            new_holon_type_descriptor(&context, "meta-relationship-type", "MetaRelationshipType")?;
        let mut declared_relationship_type = new_descriptor_holon(
            &context,
            "declared-relationship-type-for-source-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_relationship_type = new_descriptor_holon(
            &context,
            "inverse-relationship-type-for-source-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;

        let mut source_type = new_relationship_descriptor_holon(
            &context,
            "source-type",
            "SourceType",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        let mut source_for = new_relationship_descriptor_holon(
            &context,
            "source-for",
            "SourceFor",
            (&type_descriptor).into(),
            (&meta_relationship_type).into(),
        )?;
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

        meta_relationship_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&source_type).into()],
        )?;

        let mut concrete_relationship = new_relationship_descriptor_holon(
            &context,
            "affords-operator",
            "AffordsOperator",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        concrete_relationship
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&source_type).into()])?;
        concrete_relationship.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.into()],
        )?;

        assert!(matches!(
            resolve_inverse_relationship_name(
                &(&concrete_relationship).into(),
                &RelationshipName(MapString("SourceType".to_string())),
            ),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "SourceType"
        ));
        Ok(())
    }

    #[test]
    fn resolves_instance_relationship_through_described_by_contract() -> Result<(), HolonError> {
        let context = build_context();
        let meta_holon_type =
            new_holon_type_descriptor(&context, "meta-holon-type", "MetaHolonType")?;
        let mut holon_space_type =
            new_holon_type_descriptor(&context, "holon-space-type", "HolonSpace")?;
        holon_space_type.with_descriptor((&meta_holon_type).into())?;
        let transaction_type =
            new_holon_type_descriptor(&context, "transaction-type", "Transaction")?;
        let declared_type = new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;

        let declared_transient = new_relationship_descriptor_holon(
            &context,
            "affords-transaction-model",
            "AffordsTransactionModel",
            (&holon_space_type).into(),
            (&transaction_type).into(),
        )?;
        let inverse_transient = new_relationship_descriptor_holon(
            &context,
            "transaction-model-afforded-by",
            "TransactionModelAffordedBy",
            (&transaction_type).into(),
            (&holon_space_type).into(),
        )?;
        let mut declared = declared_transient;
        let mut inverse = inverse_transient;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        declared
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&inverse).into()])?;

        holon_space_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;
        let mut holon_space = new_test_holon(&context, "holon-space-instance")?;
        holon_space.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![(&holon_space_type).into()],
        )?;

        let inverse_name = resolve_inverse_relationship_name(
            &(&holon_space).into(),
            &RelationshipName(MapString("AffordsTransactionModel".to_string())),
        )?;

        assert_eq!(
            inverse_name,
            RelationshipName(MapString("TransactionModelAffordedBy".to_string()))
        );
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
        let inverse_type = new_descriptor_holon(
            &fixture.context,
            "wrong-kind-inverse-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut wrong_kind_relationship = new_descriptor_holon(
            &fixture.context,
            "wrong-kind-authored-by",
            "AuthoredBy",
            "Relationship",
        )?;
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
