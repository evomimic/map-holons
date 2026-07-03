//! Instance-level relationship availability.
//!
//! Available relationships are the runtime-state-filtered subset of a source
//! holon's effective outbound relationship surface:
//!
//! | Source reference state          | Declared | Inverse |
//! | ------------------------------- | -------- | ------- |
//! | `SmartReference`                | yes      | yes     |
//! | committed `StagedReference`     | yes      | yes     |
//! | uncommitted `StagedReference`   | yes      | no      |
//! | `TransientReference`            | yes      | no      |
//!
//! The declared side is collected from `effective_descriptor_lineage`, so
//! descriptor holon sources include declared relationships from their own
//! `Extends` lineage. The inverse side remains anchored on the source holon's
//! `DescribedBy` descriptor and is consulted only for committed sources.

use crate::descriptors::{
    accessor_helpers, effective_descriptor_lineage,
    effective_relationships::collect_declared_from_anchors, Descriptor, QualifiedRelationship,
    RelationshipDescriptor, RelationshipDirection,
};
use crate::reference_layer::{readable_impl::ReadableHolonImpl, ReadableHolon};
use core_types::HolonError;

pub(crate) fn available_relationships<T>(
    source_ref: &T,
) -> Result<Vec<QualifiedRelationship>, HolonError>
where
    T: ReadableHolonImpl + ?Sized,
{
    let source_reference = source_ref.holon_reference_impl();
    let mut relationships = collect_declared_from_anchors(
        effective_descriptor_lineage(&source_reference)?.into_iter().map(Ok),
        || accessor_helpers::descriptor_label(&source_reference),
    )?
    .into_iter()
    .map(|declared| QualifiedRelationship {
        descriptor: RelationshipDescriptor::from_holon(declared.holon().clone()),
        descriptor_direction: RelationshipDirection::Declared,
    })
    .collect::<Vec<_>>();

    if source_ref.is_committed_source_impl()? {
        relationships.extend(
            source_ref.holon_descriptor()?.effective_inverse_relationships()?.into_iter().map(
                |inverse| QualifiedRelationship {
                    descriptor: RelationshipDescriptor::from_holon(inverse.holon().clone()),
                    descriptor_direction: RelationshipDirection::Inverse,
                },
            ),
        );
    }

    Ok(relationships)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::Holon;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::descriptors::RelationshipDirection;
    use crate::reference_layer::{
        readable_impl::ReadableHolonImpl, HolonReference, ReadableHolon, SmartReference,
        StagedReference, WritableHolon,
    };
    use core_types::{HolonError, HolonId, LocalId};
    use std::sync::Arc;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    struct AvailabilityFixture {
        context: Arc<crate::core_shared_objects::transactions::TransactionContext>,
        target_type: StagedReference,
    }

    fn qualified_names(
        relationships: &[QualifiedRelationship],
    ) -> Result<Vec<(String, RelationshipDirection)>, HolonError> {
        relationships
            .iter()
            .map(|qualified| {
                Ok((
                    qualified.descriptor.base_relationship_name()?.to_string(),
                    qualified.descriptor_direction,
                ))
            })
            .collect()
    }

    fn build_availability_fixture(key_prefix: &str) -> Result<AvailabilityFixture, HolonError> {
        let context = build_context();
        let declared_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            &format!("{key_prefix}-declared-type"),
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?)?;
        let inverse_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            &format!("{key_prefix}-inverse-type"),
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let mut source_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            &format!("{key_prefix}-book-type"),
            "BookType",
        )?)?;
        let mut target_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            &format!("{key_prefix}-person-type"),
            "PersonType",
        )?)?;
        let club_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            &format!("{key_prefix}-club-type"),
            "ClubType",
        )?)?;

        let authored_by_transient = new_relationship_descriptor_holon(
            &context,
            &format!("{key_prefix}-authored-by"),
            "AuthoredBy",
            HolonReference::from(&source_type),
            HolonReference::from(&target_type),
        )?;
        let authors_transient = new_relationship_descriptor_holon(
            &context,
            &format!("{key_prefix}-authors"),
            "Authors",
            HolonReference::from(&target_type),
            HolonReference::from(&source_type),
        )?;
        let member_of_transient = new_relationship_descriptor_holon(
            &context,
            &format!("{key_prefix}-member-of"),
            "MemberOf",
            HolonReference::from(&target_type),
            HolonReference::from(&club_type),
        )?;
        let mut authored_by = context.mutation().stage_new_holon(authored_by_transient)?;
        let mut authors = context.mutation().stage_new_holon(authors_transient)?;
        let mut member_of = context.mutation().stage_new_holon(member_of_transient)?;

        authored_by.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&declared_type)],
        )?;
        member_of.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&declared_type)],
        )?;
        authors.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        authored_by
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&authors).into()])?;
        authors
            .add_related_holons(CoreRelationshipTypeName::InverseOf, vec![(&authored_by).into()])?;

        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&authored_by).into()],
        )?;
        target_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![member_of.into()],
        )?;
        target_type
            .add_related_holons(CoreRelationshipTypeName::TargetOf, vec![authored_by.into()])?;

        Ok(AvailabilityFixture { context, target_type })
    }

    fn mark_committed(
        context: &Arc<crate::core_shared_objects::transactions::TransactionContext>,
        staged: &StagedReference,
    ) -> Result<(), HolonError> {
        let rc_holon = staged.get_holon_to_commit(&context)?;
        let mut holon = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        match &mut *holon {
            Holon::Staged(staged_holon) => staged_holon.to_committed(LocalId(vec![7, 8, 9])),
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }
    }

    #[test]
    fn transient_source_returns_declared_only() -> Result<(), HolonError> {
        let fixture = build_availability_fixture("avail-transient-ref")?;
        let mut source = new_test_holon(&fixture.context, "avail-transient-person")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&fixture.target_type)],
        )?;

        let names = qualified_names(&source.available_relationships()?)?;

        assert_eq!(names, vec![("MemberOf".to_string(), RelationshipDirection::Declared)]);
        Ok(())
    }

    #[test]
    fn uncommitted_staged_source_returns_declared_only() -> Result<(), HolonError> {
        let fixture = build_availability_fixture("avail-uncommitted-ref")?;
        let source = new_test_holon(&fixture.context, "avail-uncommitted-person")?;
        let mut staged_source = fixture.context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&fixture.target_type)],
        )?;

        let names = qualified_names(&staged_source.available_relationships()?)?;

        assert_eq!(names, vec![("MemberOf".to_string(), RelationshipDirection::Declared)]);
        Ok(())
    }

    #[test]
    fn committed_staged_source_returns_declared_and_inverse() -> Result<(), HolonError> {
        let fixture = build_availability_fixture("avail-committed-ref")?;
        let source = new_test_holon(&fixture.context, "avail-committed-person")?;
        let mut staged_source = fixture.context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&fixture.target_type)],
        )?;
        mark_committed(&fixture.context, &staged_source)?;

        let names = qualified_names(&staged_source.available_relationships()?)?;

        assert!(names.contains(&("MemberOf".to_string(), RelationshipDirection::Declared)));
        assert!(names.contains(&("Authors".to_string(), RelationshipDirection::Inverse)));
        assert_eq!(names.len(), 2);
        Ok(())
    }

    #[test]
    fn descriptor_holon_source_includes_own_lineage_declared_relationships(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let type_descriptor =
            new_holon_type_descriptor(&context, "avail-type-descriptor", "TypeDescriptor")?;
        let mut meta_relationship_type = new_holon_type_descriptor(
            &context,
            "avail-meta-relationship-type",
            "MetaRelationshipType",
        )?;
        let mut declared_relationship_type = new_descriptor_holon(
            &context,
            "avail-declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut source_type = new_relationship_descriptor_holon(
            &context,
            "avail-source-type",
            "SourceType",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        let target_descriptor = new_holon_type_descriptor(&context, "avail-target-type", "Target")?;
        let mut relationship_descriptor = new_relationship_descriptor_holon(
            &context,
            "avail-affords-operator",
            "AffordsOperator",
            (&meta_relationship_type).into(),
            (&target_descriptor).into(),
        )?;

        meta_relationship_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&source_type).into()],
        )?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&declared_relationship_type).into()],
        )?;
        declared_relationship_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&meta_relationship_type).into()],
        )?;
        relationship_descriptor.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_relationship_type.into()],
        )?;
        relationship_descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.into()],
        )?;

        let names = qualified_names(&relationship_descriptor.available_relationships()?)?;

        assert!(names.contains(&("SourceType".to_string(), RelationshipDirection::Declared)));
        Ok(())
    }

    #[test]
    fn is_committed_source_impl_reports_reference_state() -> Result<(), HolonError> {
        let fixture = build_availability_fixture("commit-state-ref")?;
        let transient = new_test_holon(&fixture.context, "commit-state-transient")?;
        let staged = fixture
            .context
            .mutation()
            .stage_new_holon(new_test_holon(&fixture.context, "commit-state-staged")?)?;
        let smart = SmartReference::new_from_id(
            fixture.context.context_handle(),
            HolonId::Local(LocalId(vec![1, 2, 3])),
        );

        assert!(!transient.is_committed_source_impl()?);
        assert!(!staged.is_committed_source_impl()?);
        assert!(smart.is_committed_source_impl()?);

        let transient_ref: HolonReference = (&transient).into();
        let staged_ref: HolonReference = (&staged).into();
        let smart_ref: HolonReference = (&smart).into();

        assert!(!transient_ref.is_committed_source_impl()?);
        assert!(!staged_ref.is_committed_source_impl()?);
        assert!(smart_ref.is_committed_source_impl()?);

        mark_committed(&fixture.context, &staged)?;
        assert!(staged.is_committed_source_impl()?);
        let staged_ref: HolonReference = staged.into();
        assert!(staged_ref.is_committed_source_impl()?);

        Ok(())
    }
}
