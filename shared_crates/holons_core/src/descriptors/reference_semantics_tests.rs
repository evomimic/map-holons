use super::{test_support::*, *};
use crate::core_shared_objects::{holon::SavedHolon, transactions::TransactionContext};
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
                        BaseValue::StringValue(MapString(match i {
                            0 => "TypeDescriptor".into(),
                            2 => "Schema.HolonType".into(),
                            _ => format!("saved-{i}"),
                        })),
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
fn saved_lineage_ignores_staged_successor_but_explicit_reference_uses_it() -> Result<(), HolonError>
{
    let (context, saved) = saved_fixture();
    let snapshot = new_kind_descriptor(&context, "successor", Some(&saved[4]), true)?;
    let mut update =
        stage_update_snapshot(&context, saved[1].holon_id()?.local_id().clone(), snapshot)?;
    update.with_property_value("Description", "changed")?;
    assert_eq!(ancestors(&saved[2])?, vec![saved[2].clone(), saved[1].clone(), saved[0].clone()]);
    let diagnosis = ExtendsLineageDiagnosis::assess(&saved[2])?;
    assert!(diagnosis.defects.is_empty());
    let update: HolonReference = update.into();
    assert_eq!(ancestors(&update)?, vec![update.clone(), saved[4].clone()]);
    assert!(!same_definition(&saved[1], &update)?);
    let mut explicit = new_test_holon(&context, "explicit")?;
    explicit.add_related_holons(R::Extends, vec![update.clone()])?;
    assert_eq!(ancestors(&explicit.into())?[1], update);
    Ok(())
}

#[test]
fn exact_contract_members_applicability_and_ownership_preserve_provenance() -> Result<(), HolonError>
{
    let (context, saved) = saved_fixture();
    let mut snapshot = new_kind_descriptor(&context, "successor", Some(&saved[0]), false)?;
    snapshot.add_related_holons(R::InstanceProperties, vec![saved[2].clone()])?;
    snapshot.add_related_holons(R::ApplicableToDescriptorTypes, vec![saved[0].clone()])?;
    snapshot.add_related_holons(R::ComponentOf, vec![saved[3].clone()])?;
    let mut update =
        stage_update_snapshot(&context, saved[1].holon_id()?.local_id().clone(), snapshot)?;
    update.with_property_value("Description", "changed")?;
    let update: HolonReference = update.into();
    assert!(ContractContributions::resolve(&saved[1])?.properties.is_empty());
    let contract = ContractContributions::resolve(&update)?;
    assert_eq!(contract.properties[0].member, saved[2]);
    assert_eq!(contract.properties[0].declared_on, update);
    assert!(applicable_descriptor_types(&saved[1])?.is_empty());
    assert!(constraint_applies_to(&update, &saved[2])?);
    assert!(!constraint_applies_to(&saved[1], &saved[2])?);
    assert!(matches!(
        resolve_schema_ownership(&saved[1], SchemaOwnershipKind::Component)?,
        SchemaOwnershipResolution::Missing
    ));
    assert!(
        matches!(resolve_schema_ownership(&update, SchemaOwnershipKind::Component)?, SchemaOwnershipResolution::OwnedBy(owner) if owner == saved[3])
    );
    Ok(())
}

#[test]
fn plain_apis_preserve_backend_errors_and_reject_foreign_mutable_lineage() -> Result<(), HolonError>
{
    let (context, _) = saved_fixture();
    let missing = HolonReference::smart_from_id(
        context.space_read_handle(),
        HolonId::Local(LocalId(vec![99; 39])),
    );
    assert!(matches!(ExtendsLineageDiagnosis::assess(&missing), Err(HolonError::HolonNotFound(_))));
    let other = build_context();
    let foreign: HolonReference = new_test_holon(&other, "foreign")?.into();
    let mut local = new_test_holon(&context, "local")?;
    local.add_related_holons(R::Extends, vec![foreign])?;
    assert!(matches!(ancestors(&local.into()), Err(HolonError::CrossTransactionReference { .. })));
    Ok(())
}

#[test]
fn field_shaped_backend_errors_remain_kind_read_failures() -> Result<(), HolonError> {
    for error in [
        HolonError::EmptyField("backend".into()),
        HolonError::UnexpectedValueType("backend".into(), "reference".into()),
    ] {
        let id = HolonId::Local(LocalId(vec![42; 39]));
        let context = build_context_with_read_errors(
            vec![],
            HashMap::new(),
            HashMap::from([(id.clone(), error.clone())]),
        );
        let root: HolonReference =
            new_kind_descriptor(&context, "TypeDescriptor", None, false)?.into();
        let mut child = new_kind_descriptor(&context, "child", Some(&root), false)?;
        child.add_related_holons(R::DescribedBy, vec![root.clone()])?;
        let child: HolonReference = child.into();
        let mut prerequisites = StructuralPrerequisites::assess(&child)?;
        // The product contains a known lineage; inject a backend failure for its supporting
        // definition fields rather than replacing traversal with a custom reader.
        let unreadable = HolonReference::smart_from_id(context.space_read_handle(), id.clone());
        prerequisites.subject_lineage.lineage.insert(1, unreadable);
        prerequisites.subject_lineage.member_keys.insert(1, None);
        let roots = DescriptorKindSemantics::new(&context);
        for result in [
            roots
                .assess_instance_type_kind(prerequisites.subject_lineage.valid_lineage().unwrap())
                .map(|_| ()),
            roots.assess_describing_lineages_compatible(&prerequisites).map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(KindResolutionError::Read(observed)) if observed == error)
            );
        }
    }
    Ok(())
}

#[test]
fn universal_contract_uses_explicit_meta_type_with_split_bootstrap_and_successors(
) -> Result<(), HolonError> {
    // One root is saved and the meta-type is still a bootstrap create.
    let root_id = HolonId::Local(LocalId(vec![50; 39]));
    let saved_root = SavedHolon::new(
        root_id.local_id().clone(),
        PropertyMap::from([(
            "Key".to_property_name(),
            BaseValue::StringValue(MapString("TypeDescriptor".into())),
        )]),
        None,
        MapInteger(1),
    );
    let context = build_context_with_saved_holons(vec![saved_root], HashMap::new());
    let root = HolonReference::smart_from_id(context.space_read_handle(), root_id.clone());
    let member =
        context.mutation().stage_new_holon(new_test_holon(&context, "universal-member")?)?;
    let mut meta = new_test_holon(&context, "MetaTypeDescriptor.HolonType")?;
    meta.add_related_holons(R::InstanceProperties, vec![member.clone().into()])?;
    let meta = context.mutation().stage_new_holon(meta)?;
    let mut successor = stage_update_snapshot(
        &context,
        root_id.local_id().clone(),
        new_test_holon(&context, "TypeDescriptor")?,
    )?;
    successor.with_property_value("Description", "new root definition")?;
    let contract = UniversalDescriptorContract::from_meta_type(&context, &meta.into())?;
    let mut descriptor = new_test_holon(&context, "abstract-descriptor")?;
    descriptor.with_property_value("IsAbstractType", true)?;
    descriptor.add_related_holons(R::Extends, vec![root])?;
    let descriptor = descriptor.into();
    let unrelated = new_test_holon(&context, "unrelated")?.into();
    assert!(!contract.enforce_minimum(&descriptor, true, &unrelated)?);
    assert!(contract.enforce_minimum(&descriptor, true, &member.into())?);
    Ok(())
}

#[test]
fn universal_contract_does_not_adopt_successor_member_content() -> Result<(), HolonError> {
    let ids: Vec<_> = (60..=61).map(|n| HolonId::Local(LocalId(vec![n; 39]))).collect();
    let saved = ["MetaTypeDescriptor.HolonType", "saved-member"]
        .iter()
        .zip(&ids)
        .map(|(key, id)| {
            SavedHolon::new(
                id.local_id().clone(),
                PropertyMap::from([(
                    "Key".to_property_name(),
                    BaseValue::StringValue(MapString((*key).into())),
                )]),
                None,
                MapInteger(1),
            )
        })
        .collect();
    let context = build_context_with_saved_holons(
        saved,
        HashMap::from([(
            (ids[0].clone(), R::InstanceProperties.to_relationship_name()),
            vec![ids[1].clone()],
        )]),
    );
    let root = context.mutation().stage_new_holon(new_test_holon(&context, "TypeDescriptor")?)?;
    let successor_member: HolonReference = new_test_holon(&context, "successor-member")?.into();
    let mut snapshot = new_test_holon(&context, "MetaTypeDescriptor.HolonType")?;
    snapshot.add_related_holons(R::InstanceProperties, vec![successor_member.clone()])?;
    let mut successor = stage_update_snapshot(&context, ids[0].local_id().clone(), snapshot)?;
    successor.with_property_value("Description", "new contract")?;
    let meta = HolonReference::smart_from_id(context.space_read_handle(), ids[0].clone());
    let contract = UniversalDescriptorContract::from_meta_type(&context, &meta)?;
    let mut subject = new_test_holon(&context, "abstract")?;
    subject.with_property_value("IsAbstractType", true)?;
    subject.add_related_holons(R::Extends, vec![root.into()])?;
    let subject = subject.into();
    let saved_member = HolonReference::smart_from_id(context.space_read_handle(), ids[1].clone());
    assert!(contract.enforce_minimum(&subject, true, &saved_member)?);
    assert!(!contract.enforce_minimum(&subject, true, &successor_member)?);
    Ok(())
}
