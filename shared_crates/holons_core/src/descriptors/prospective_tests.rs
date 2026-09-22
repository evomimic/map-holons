use super::{test_support::*, *};
use crate::core_shared_objects::{holon::SavedHolon, transactions::TransactionContext, Holon};
use crate::{HolonReference, ReadableHolon, WritableHolon};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyMap};
use std::{collections::HashMap, sync::Arc};
use type_names::{CoreRelationshipTypeName as R, ToPropertyName, ToRelationshipName};

fn saved_fixture() -> (Arc<TransactionContext>, Vec<HolonReference>) {
    let ids: Vec<_> = (1..=5).map(|i| HolonId::Local(LocalId(vec![i; 39]))).collect();
    let holons = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            SavedHolon::new(
                id.local_id().clone(),
                PropertyMap::from([
                    (
                        "Key".to_property_name(),
                        BaseValue::StringValue(MapString(format!("saved-{i}"))),
                    ),
                    (
                        "DefinesInstanceTypeKind".to_property_name(),
                        BaseValue::BooleanValue(false.into()),
                    ),
                ]),
                None,
                MapInteger(1),
            )
        })
        .collect();
    let context = build_context_with_saved_holons(
        holons,
        HashMap::from([
            ((ids[1].clone(), R::Extends.to_relationship_name()), vec![ids[0].clone()]),
            ((ids[2].clone(), R::Extends.to_relationship_name()), vec![ids[1].clone()]),
            ((ids[3].clone(), R::DescribedBy.to_relationship_name()), vec![ids[2].clone()]),
        ]),
    );
    let references = ids
        .into_iter()
        .map(|id| HolonReference::smart_from_id(context.space_read_handle(), id))
        .collect();
    (context, references)
}

#[test]
fn selection_uses_full_ancestry_and_never_selects_contested_content() -> Result<(), HolonError> {
    let (context, saved) = saved_fixture();
    let source = saved[0].holon_id()?.local_id().clone();
    let first =
        stage_update_snapshot(&context, source.clone(), new_test_holon(&context, "saved-0")?)?;
    let create = context.mutation().stage_new_holon(new_test_holon(&context, "saved-0")?)?;
    let reader = ProspectiveDescriptorReader::new(&context, &[first.clone(), create.clone()])?;
    assert!(matches!(reader.selection(&saved[0])?, ProspectiveSelection::Replaced(_)));
    assert!(matches!(reader.selection(&create.into())?, ProspectiveSelection::Saved));
    assert!(!same_definition(&saved[0], &first.clone().into()));
    assert!(same_definition(&reader.select(&saved[0]).unwrap(), &first.clone().into()));
    assert!(matches!(reader.selection(&saved[4])?, ProspectiveSelection::Saved));
    // Content equality and an unchanged ForUpdate state do not license choosing a winner.
    let second = stage_update_snapshot(&context, source, new_test_holon(&context, "saved-0")?)?;
    let reader = ProspectiveDescriptorReader::new(
        &context,
        &[second.clone(), first.clone(), first.clone()],
    )?;
    for reference in [&saved[0], &first.clone().into(), &second.clone().into()] {
        assert!(
            matches!(reader.selection(reference)?, ProspectiveSelection::Contested(members) if members.len() == 2)
        );
        assert!(matches!(reader.select(reference), Err(AssessmentReadError::Contested { .. })));
    }
    assert!(matches!(
        resolve_core_descriptor_with_reader(&context, "saved-0", &reader),
        Err(AssessmentReadError::Operational(HolonError::DuplicateError(..)))
    ));
    assert_eq!(
        crate::ProspectiveIdentity::for_reference(&first.into(), &context)?,
        crate::ProspectiveIdentity::for_reference(&second.into(), &context)?
    );
    Ok(())
}

#[test]
fn replacement_parent_content_governs_lineage_and_cycles() -> Result<(), HolonError> {
    let (context, saved) = saved_fixture();
    let mut replacement = new_kind_descriptor(&context, "saved-1", Some(&saved[4]), true)?;
    describe(&mut replacement, &saved[3])?;
    let update =
        stage_update_snapshot(&context, saved[1].holon_id()?.local_id().clone(), replacement)?;
    let reader = ProspectiveDescriptorReader::new(&context, std::slice::from_ref(&update))?;
    // The normal saved walk still ends at the old root. Prospective reads use the new parent.
    assert!(ExtendsLineageDiagnosis::assess(&saved[2], &saved[0])?.defects.is_empty());
    let diagnosis =
        ExtendsLineageDiagnosis::assess_with_reader(&saved[2], &saved[0], &reader).unwrap();
    assert!(
        matches!(diagnosis.defects.as_slice(), [ExtendsLineageDefect::WrongTermination { terminal }] if same_definition(terminal, &saved[4]))
    );
    let diagnosis =
        ExtendsLineageDiagnosis::assess_with_reader(&saved[2], &saved[4], &reader).unwrap();
    let roots = DescriptorKindRoots::from_resolved(
        &context,
        saved[4].clone(),
        saved[4].clone(),
        saved[3].clone(),
        saved[3].clone(),
    )?
    .with_reader(&reader);
    assert!(same_definition(
        &roots.instance_type_kind(diagnosis.valid_lineage().unwrap()).unwrap().unwrap(),
        &update.clone().into()
    ));
    assert!(same_definition(
        &roots.required_describing_category(diagnosis.valid_lineage().unwrap()).unwrap().unwrap(),
        &saved[3]
    ));
    // A prospective self-cycle through the old saved identity must be detected once.
    let cyclic = new_kind_descriptor(&context, "cycle", Some(&saved[1]), false)?;
    update.abandon_staged_changes(&context)?;
    let cycle = stage_update_snapshot(&context, saved[1].holon_id()?.local_id().clone(), cyclic)?;
    let reader = ProspectiveDescriptorReader::new(&context, &[update, cycle])?;
    let diagnosis =
        ExtendsLineageDiagnosis::assess_with_reader(&saved[2], &saved[0], &reader).unwrap();
    assert_eq!(diagnosis.lineage.len(), 2);
    assert!(matches!(diagnosis.defects.as_slice(), [ExtendsLineageDefect::Cycle { .. }]));
    Ok(())
}

#[test]
fn prospective_ownership_constraints_and_contributions_share_selection() -> Result<(), HolonError> {
    let (context, saved) = saved_fixture();
    let mut schema_type = new_kind_descriptor(&context, "schema-type", Some(&saved[0]), true)?;
    schema_type.add_related_holons(R::ApplicableToDescriptorTypes, vec![saved[0].clone()])?;
    let schema_update =
        stage_update_snapshot(&context, saved[2].holon_id()?.local_id().clone(), schema_type)?;
    let mut descriptor = new_kind_descriptor(&context, "component", Some(&saved[0]), false)?;
    descriptor.add_related_holons(R::ComponentOf, vec![saved[3].clone()])?;
    descriptor.add_related_holons(R::InstanceProperties, vec![saved[2].clone()])?;
    descriptor.add_related_holons(R::RuleOf, vec![saved[3].clone()])?;
    let update =
        stage_update_snapshot(&context, saved[1].holon_id()?.local_id().clone(), descriptor)?;
    let reader = ProspectiveDescriptorReader::new(&context, &[update, schema_update.clone()])?;
    for kind in [SchemaOwnershipKind::Component, SchemaOwnershipKind::Rule] {
        assert!(matches!(
            resolve_schema_ownership_with_reader(
                &saved[1],
                kind,
                &schema_update.clone().into(),
                &reader
            )
            .unwrap(),
            SchemaOwnershipResolution::OwnedBy(_)
        ));
    }
    let contributions = ContractContributions::resolve_with_reader(&saved[1], &reader).unwrap();
    assert_eq!(contributions.properties.len(), 1);
    assert!(matches!(&contributions.properties[0].member, HolonReference::Staged(_)));
    assert!(matches!(&contributions.properties[0].declared_on, HolonReference::Staged(_)));
    assert!(constraint_applies_to_with_reader(&saved[2], &saved[1], &reader).unwrap());
    assert!(applicable_descriptor_types(&saved[2])?.is_empty());
    Ok(())
}

#[test]
fn graph_only_competitors_block_reads_and_retry_excludes_finished_entries() -> Result<(), HolonError>
{
    let (context, saved) = saved_fixture();
    let source = saved[1].holon_id()?.local_id().clone();
    let first =
        stage_update_snapshot(&context, source.clone(), new_test_holon(&context, "saved-1")?)?;
    let second = stage_update_snapshot(&context, source, new_test_holon(&context, "saved-1")?)?;
    {
        let rc = second.get_holon_to_commit(&context)?;
        let mut holon = rc.write().unwrap();
        let Holon::Staged(holon) = &mut *holon else { panic!("staged snapshot") };
        holon.note_relationship_mutation(false)?;
    }
    let reader = ProspectiveDescriptorReader::new(&context, &[first.clone(), second.clone()])?;
    assert!(matches!(
        ExtendsLineageDiagnosis::assess_with_reader(&saved[2], &saved[0], &reader),
        Err(AssessmentReadError::Contested { .. })
    ));
    assert!(matches!(
        resolve_core_descriptor_with_reader(&context, "saved-1", &reader),
        Err(AssessmentReadError::Contested { .. })
    ));
    assert!(ExtendsLineageDiagnosis::assess_with_reader(&saved[4], &saved[0], &reader)
        .unwrap()
        .defects
        .is_empty());
    second.abandon_staged_changes(&context)?;
    let retry = ProspectiveDescriptorReader::new(&context, &[first.clone(), second.clone()])?;
    assert_eq!(retry.contested_groups().count(), 0);
    assert!(matches!(retry.selection(&saved[1])?, ProspectiveSelection::Replaced(_)));
    {
        let rc = first.get_holon_to_commit(&context)?;
        let mut holon = rc.write().unwrap();
        let Holon::Staged(holon) = &mut *holon else { panic!("staged snapshot") };
        holon.to_committed(LocalId(vec![9; 39]))?;
    }
    assert!(matches!(
        ProspectiveDescriptorReader::new(&context, &[first, second])?.selection(&saved[1])?,
        ProspectiveSelection::Saved
    ));
    // Another transaction may independently branch from the same saved predecessor.
    let next = context.open_isolated_transaction()?;
    let branch = stage_update_snapshot(
        &next,
        saved[1].holon_id()?.local_id().clone(),
        new_test_holon(&next, "branch")?,
    )?;
    assert_eq!(ProspectiveDescriptorReader::new(&next, &[branch])?.contested_groups().count(), 0);
    Ok(())
}

#[test]
fn prospective_reads_preserve_operational_errors_and_reject_foreign_mutable_refs(
) -> Result<(), HolonError> {
    let (context, saved) = saved_fixture();
    let reader = ProspectiveDescriptorReader::new(&context, &[])?;
    let other = build_context();
    let foreign = new_test_holon(&other, "foreign")?;
    assert!(matches!(
        reader.select(&foreign.into()),
        Err(AssessmentReadError::Operational(HolonError::CrossTransactionReference { .. }))
    ));
    let missing = HolonReference::smart_from_id(
        context.space_read_handle(),
        HolonId::Local(LocalId(vec![99; 39])),
    );
    assert!(matches!(
        ExtendsLineageDiagnosis::assess_with_reader(&missing, &saved[0], &reader),
        Err(AssessmentReadError::Operational(_))
    ));
    Ok(())
}

#[test]
fn lineage_selects_each_visited_reference_once() -> Result<(), HolonError> {
    struct CountingReader(std::cell::Cell<usize>);
    impl DescriptorReader for CountingReader {
        type Error = HolonError;
        fn select(&self, reference: &HolonReference) -> Result<HolonReference, HolonError> {
            self.0.set(self.0.get() + 1);
            Ok(reference.clone())
        }
        fn operational_error(error: &HolonError) -> Option<&HolonError> {
            Some(error)
        }
    }
    let (_context, saved) = saved_fixture();
    let reader = CountingReader(std::cell::Cell::new(0));
    let lineage =
        walk_extends_chain_with_reader(&saved[2], &reader).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(lineage.len(), 3);
    assert_eq!(reader.0.get(), 3);
    Ok(())
}
