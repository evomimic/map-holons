use super::test_support::*;
use super::*;
use crate::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use core_types::HolonError;
use type_names::{CoreDefinitionRole, CorePropertyTypeName as P, CoreRelationshipTypeName as R};

#[test]
fn structural_products_do_not_require_classification() -> Result<(), HolonError> {
    let context = build_context();
    let root: HolonReference = new_test_holon(&context, "TypeDescriptor")?.into();
    let mut ordinary = new_test_holon(&context, "ordinary")?;
    assert!(matches!(
        resolve_describing_type(&ordinary.clone().into())?,
        DescribingTypeResolution::Missing
    ));
    assert!(ExtendsLineageDiagnosis::assess(&ordinary.clone().into())?.defects.is_empty());
    describe(&mut ordinary, &root)?;
    assert!(matches!(
        resolve_describing_type(&ordinary.clone().into())?,
        DescribingTypeResolution::Unique(_)
    ));
    let other: HolonReference = new_test_holon(&context, "other")?.into();
    describe(&mut ordinary, &other)?;
    assert!(
        matches!(resolve_describing_type(&ordinary.clone().into())?, DescribingTypeResolution::Multiple(targets) if targets.len() == 2)
    );
    ordinary.add_related_holons(R::Extends, vec![root.clone(), other])?;
    let diagnosis = ExtendsLineageDiagnosis::assess(&ordinary.into())?;
    assert!(matches!(
        diagnosis.defects.as_slice(),
        [ExtendsLineageDefect::MultipleParents { count: 2, .. }]
    ));
    Ok(())
}

#[test]
fn lineage_drains_pending_errors_and_checks_termination_and_root() -> Result<(), HolonError> {
    let context = build_context();
    let mut root = new_test_holon(&context, "TypeDescriptor")?;
    let mut first = new_test_holon(&context, "first")?;
    let mut second = new_test_holon(&context, "second")?;
    first.add_related_holons(R::Extends, vec![second.clone().into()])?;
    let diagnosis = ExtendsLineageDiagnosis::assess(&first.clone().into())?;
    assert!(matches!(
        diagnosis.defects.as_slice(),
        [ExtendsLineageDefect::WrongTermination { .. }]
    ));
    second.add_related_holons(R::Extends, vec![first.clone().into()])?;
    assert!(equals_or_extends(&first.clone().into(), &first.clone().into())?);
    let diagnosis = ExtendsLineageDiagnosis::assess(&first.clone().into())?;
    assert_eq!(diagnosis.lineage.len(), 2);
    assert!(matches!(diagnosis.defects.as_slice(), [ExtendsLineageDefect::Cycle { .. }]));
    root.add_related_holons(R::Extends, vec![first.into()])?;
    let diagnosis = ExtendsLineageDiagnosis::assess(&root.clone().into())?;
    assert!(diagnosis
        .defects
        .iter()
        .any(|defect| matches!(defect, ExtendsLineageDefect::RootHasParent)));
    Ok(())
}

#[test]
fn a_unique_describer_does_not_establish_its_lineage_validity() -> Result<(), HolonError> {
    let context = build_context();
    let mut descriptor = new_test_holon(&context, "descriptor")?;
    descriptor.add_related_holons(R::Extends, vec![descriptor.clone().into()])?;
    let mut subject = new_test_holon(&context, "subject")?;
    describe(&mut subject, &descriptor.into())?;
    let prerequisites = StructuralPrerequisites::assess(&subject.into())?;
    assert!(prerequisites.subject_lineage.defects.is_empty());
    assert!(matches!(
        prerequisites.governing_lineage.unwrap().defects.as_slice(),
        [ExtendsLineageDefect::Cycle { .. }]
    ));
    Ok(())
}

struct KindFixture {
    type_descriptor: HolonReference,
    holon_type: HolonReference,
    meta_type: HolonReference,
    meta_holon_type: HolonReference,
    semantics: DescriptorKindSemantics,
}

fn kind_fixture(
    context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
) -> Result<KindFixture, HolonError> {
    // Transient construction permits the canonical self-describing meta-type.
    let mut root = new_kind_descriptor(context, "TypeDescriptor", None, false)?;
    let mut holon =
        new_kind_descriptor(context, "HolonType.TypeDescriptor", Some(&root.clone().into()), true)?;
    let meta = new_kind_descriptor(
        context,
        "MetaTypeDescriptor.HolonType",
        Some(&holon.clone().into()),
        false,
    )?;
    let mut meta_holon = new_kind_descriptor(
        context,
        "MetaHolonType.MetaTypeDescriptor",
        Some(&meta.clone().into()),
        false,
    )?;
    let self_reference = meta_holon.clone().into();
    describe(&mut meta_holon, &self_reference)?;
    describe(&mut root, &self_reference)?;
    describe(&mut holon, &self_reference)?;
    Ok(KindFixture {
        type_descriptor: root.into(),
        holon_type: holon.into(),
        meta_type: meta.into(),
        meta_holon_type: meta_holon.into(),
        semantics: DescriptorKindSemantics::new(context),
    })
}

#[test]
fn kind_semantics_need_no_core_catalog() -> Result<(), HolonError> {
    let context = build_context();
    let kinds = DescriptorKindSemantics::new(&context);
    let ordinary: HolonReference = new_test_holon(&context, "ordinary")?.into();
    let diagnosis = ExtendsLineageDiagnosis::assess(&ordinary)?;
    let lineage = diagnosis.valid_lineage().unwrap();
    assert!(!kinds.is_descriptor(lineage)?);
    assert!(kinds.instance_type_kind(lineage)?.is_none());
    Ok(())
}

#[test]
fn local_anchor_accessor_does_not_inherit() -> Result<(), HolonError> {
    let context = build_context();
    let parent = new_kind_descriptor(&context, "parent", None, true)?;
    let mut child = new_test_holon(&context, "child")?;
    child.add_related_holons(R::Extends, vec![parent.into()])?;
    assert!(matches!(
        TypeHeader::new(&child.clone().into()).defines_instance_type_kind(),
        Err(HolonError::EmptyField(_))
    ));
    child.with_property_value(P::DefinesInstanceTypeKind, "true")?;
    assert!(matches!(
        TypeHeader::new(&child.clone().into()).defines_instance_type_kind(),
        Err(HolonError::UnexpectedValueType(..))
    ));
    child.with_property_value(P::DefinesInstanceTypeKind, false)?;
    assert!(!TypeHeader::new(&child.into()).defines_instance_type_kind()?);
    Ok(())
}

#[test]
fn contributions_preserve_redeclarations_and_separate_namespaces() -> Result<(), HolonError> {
    let context = build_context();
    let mut parent = new_test_holon(&context, "parent")?;
    let mut child = new_test_holon(&context, "child")?;
    let member: HolonReference = new_descriptor_holon(&context, "one", "SameName", "")?.into();
    let collision: HolonReference = new_descriptor_holon(&context, "two", "SameName", "")?.into();
    parent.add_related_holons(R::InstanceProperties, vec![member.clone()])?;
    child.add_related_holons(R::Extends, vec![parent.clone().into()])?;
    child.add_related_holons(R::InstanceProperties, vec![member.clone(), collision.clone()])?;
    child.add_related_holons(R::InstanceRelationships, vec![collision.clone()])?;
    parent.add_related_holons(R::InstanceKeyRule, vec![member.clone()])?;
    child.add_related_holons(R::InstanceKeyRule, vec![collision.clone()])?;
    let contributions = ContractContributions::resolve(&child.clone().into())?;
    assert_eq!(contributions.properties.len(), 3);
    assert_eq!(contributions.properties[0].member, member);
    assert_eq!(contributions.properties[0].declared_on, parent.into());
    assert_eq!(contributions.properties[1].declared_on, child.clone().into());
    assert_eq!(contributions.properties[2].member, collision);
    assert_eq!(contributions.relationships.len(), 1);
    assert_eq!(effective_relationship_targets(&child.into(), R::InstanceKeyRule)?.len(), 1);
    Ok(())
}

#[test]
fn ownership_keeps_readable_defects_and_namespaces_distinct() -> Result<(), HolonError> {
    let context = build_context();
    let schema_type: HolonReference = new_test_holon(&context, "Schema.HolonType")?.into();
    let owner: HolonReference = new_schema_holon(&context, "schema", &schema_type)?.into();
    for kind in [SchemaOwnershipKind::Component, SchemaOwnershipKind::Rule] {
        let mut subject = new_test_holon(&context, "subject")?;
        assert!(matches!(
            resolve_schema_ownership(&subject.clone().into(), kind)?,
            SchemaOwnershipResolution::Missing
        ));
        own(&mut subject, &owner, kind)?;
        assert!(
            matches!(resolve_schema_ownership(&subject.clone().into(), kind)?, SchemaOwnershipResolution::OwnedBy(found) if found == owner)
        );
        own(&mut subject, &new_test_holon(&context, "invalid-owner")?.into(), kind)?;
        assert!(
            matches!(resolve_schema_ownership(&subject.into(), kind)?, SchemaOwnershipResolution::Multiple(targets) if targets.len() == 2)
        );
        let mut subject = new_test_holon(&context, "invalid")?;
        own(&mut subject, &new_test_holon(&context, "untyped-owner")?.into(), kind)?;
        assert!(matches!(
            resolve_schema_ownership(&subject.into(), kind)?,
            SchemaOwnershipResolution::InvalidOwner { .. }
        ));
    }
    let constraint_type: HolonReference = new_test_holon(&context, "constraint-type")?.into();
    let constraint = new_constraint_holon(&context, "constraint", &constraint_type, &owner)?;
    assert!(matches!(
        resolve_schema_ownership(&constraint.into(), SchemaOwnershipKind::Component)?,
        SchemaOwnershipResolution::Missing
    ));
    assert!(schema_components(&owner)?.is_empty());
    assert!(schema_rules(&owner)?.is_empty());
    Ok(())
}

#[test]
fn applicability_uses_identity_and_local_policy_without_evaluation() -> Result<(), HolonError> {
    let context = build_context();
    let anchor: HolonReference = new_test_holon(&context, "anchor")?.into();
    let unrelated: HolonReference = new_test_holon(&context, "anchor")?.into();
    let mut subtype = new_test_holon(&context, "subtype")?;
    subtype.add_related_holons(R::Extends, vec![anchor.clone()])?;
    let mut constraint_type = new_test_holon(&context, "constraint-type")?;
    constraint_type.add_related_holons(R::ApplicableToDescriptorTypes, vec![anchor.clone()])?;
    assert!(constraint_applies_to(&constraint_type.clone().into(), &subtype.into())?);
    assert!(!constraint_applies_to(&constraint_type.clone().into(), &unrelated)?);
    let mut extension = new_test_holon(&context, "extension")?;
    extension.add_related_holons(R::Extends, vec![constraint_type.into()])?;
    assert!(applicable_descriptor_types(&extension.into())?.is_empty());
    Ok(())
}

fn kind_of(
    roots: &KindFixture,
    subject: &HolonReference,
) -> Result<Option<HolonReference>, HolonError> {
    let diagnosis = ExtendsLineageDiagnosis::assess(subject)?;
    roots
        .semantics
        .instance_type_kind(diagnosis.valid_lineage().expect("fixture has a valid lineage"))
}

fn category_of(
    roots: &KindFixture,
    subject: &HolonReference,
) -> Result<Option<RequiredCategory>, HolonError> {
    let diagnosis = ExtendsLineageDiagnosis::assess(subject)?;
    roots.semantics.required_describing_category(
        diagnosis.valid_lineage().expect("fixture has a valid lineage"),
    )
}

fn compatibility_of(
    roots: &KindFixture,
    subject: &HolonReference,
) -> Result<Option<DescribingCompatibility>, HolonError> {
    let prerequisites = StructuralPrerequisites::assess(subject)?;
    roots.semantics.describing_lineages_compatible(&prerequisites)
}

#[test]
fn graph_derived_categories_support_extensions_and_self_description() -> Result<(), HolonError> {
    let context = build_context();
    let roots = kind_fixture(&context)?;
    assert!(kind_of(&roots, &roots.type_descriptor)?.is_none());
    assert_eq!(
        category_of(&roots, &roots.type_descriptor)?,
        Some(RequiredCategory::Role(CoreDefinitionRole::MetaHolonType))
    );
    assert!(
        compatibility_of(&roots, &roots.type_descriptor)?.unwrap()
            == DescribingCompatibility {
                category_matches: Some(true),
                meta_type_corresponds: true
            }
    );
    assert!(
        compatibility_of(&roots, &roots.meta_holon_type)?.unwrap()
            == DescribingCompatibility {
                category_matches: Some(true),
                meta_type_corresponds: true
            }
    );
    let mut extension_meta =
        new_kind_descriptor(&context, "unfamiliar-meta", Some(&roots.meta_type), false)?;
    describe(&mut extension_meta, &roots.meta_holon_type)?;
    let extension_meta: HolonReference = extension_meta.into();
    let mut anchor =
        new_kind_descriptor(&context, "unfamiliar-kind", Some(&roots.type_descriptor), true)?;
    describe(&mut anchor, &extension_meta)?;
    let anchor: HolonReference = anchor.into();
    let mut descriptor =
        new_kind_descriptor(&context, "unfamiliar-descriptor", Some(&anchor), false)?;
    describe(&mut descriptor, &extension_meta)?;
    let descriptor: HolonReference = descriptor.into();
    assert_eq!(kind_of(&roots, &descriptor)?, Some(anchor.clone()));
    assert_eq!(
        category_of(&roots, &descriptor)?,
        Some(RequiredCategory::Exact(extension_meta.clone()))
    );
    assert!(
        compatibility_of(&roots, &descriptor)?.unwrap()
            == DescribingCompatibility {
                category_matches: Some(true),
                meta_type_corresponds: true
            }
    );
    let mut ordinary = new_test_holon(&context, "ordinary")?;
    describe(&mut ordinary, &descriptor)?;
    assert_eq!(compatibility_of(&roots, &ordinary.into())?.unwrap().category_matches, Some(false));
    let mut valid = new_test_holon(&context, "valid")?;
    describe(&mut valid, &roots.holon_type)?;
    assert_eq!(compatibility_of(&roots, &valid.into())?.unwrap().category_matches, Some(true));
    let mut wrong_category = new_kind_descriptor(&context, "wrong-category", Some(&anchor), false)?;
    describe(&mut wrong_category, &roots.meta_holon_type)?;
    assert_eq!(
        compatibility_of(&roots, &wrong_category.into())?,
        Some(DescribingCompatibility {
            category_matches: Some(false),
            meta_type_corresponds: true
        })
    );
    let mut wrong_meta = new_test_holon(&context, "wrong-meta")?;
    describe(&mut wrong_meta, &roots.meta_holon_type)?;
    assert_eq!(
        compatibility_of(&roots, &wrong_meta.into())?,
        Some(DescribingCompatibility {
            category_matches: Some(true),
            meta_type_corresponds: false
        })
    );
    let undescribed_anchor =
        new_kind_descriptor(&context, "undescribed-anchor", Some(&roots.type_descriptor), true)?;
    let mut dependent = new_kind_descriptor(
        &context,
        "dependent-on-anchor",
        Some(&undescribed_anchor.into()),
        false,
    )?;
    describe(&mut dependent, &roots.meta_holon_type)?;
    assert_eq!(
        compatibility_of(&roots, &dependent.into())?,
        Some(DescribingCompatibility { category_matches: None, meta_type_corresponds: true })
    );
    let mut malformed = new_kind_descriptor(&context, "malformed", None, true)?;
    malformed.add_related_holons(R::Extends, vec![malformed.clone().into()])?;
    let malformed = ExtendsLineageDiagnosis::assess(&malformed.into())?;
    assert!(malformed.valid_lineage().is_none());
    assert!(
        matches!(malformed.defects.as_slice(), [ExtendsLineageDefect::Cycle { repeated_descriptor, .. }] if !repeated_descriptor.is_empty())
    );
    let mut no_describer = new_test_holon(&context, "no-describer")?;
    no_describer.add_related_holons(R::Extends, vec![roots.type_descriptor.clone()])?;
    let prerequisites = StructuralPrerequisites::assess(&no_describer.into())?;
    assert!(roots.semantics.describing_lineages_compatible(&prerequisites)?.is_none());
    Ok(())
}

#[test]
fn saved_navigation_reads_distinct_collections_and_operational_failures_propagate(
) -> Result<(), HolonError> {
    use crate::core_shared_objects::holon::SavedHolon;
    use base_types::MapInteger;
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::ToRelationshipName;
    let ids: Vec<_> = (0..4).map(|index| HolonId::Local(LocalId(vec![index; 39]))).collect();
    let saved = (0..4)
        .map(|index| {
            SavedHolon::new(LocalId(vec![index; 39]), PropertyMap::new(), None, MapInteger(1))
        })
        .collect();
    let relationships = HashMap::from([
        ((ids[0].clone(), R::Components.to_relationship_name()), vec![ids[1].clone()]),
        ((ids[0].clone(), R::Rules.to_relationship_name()), vec![ids[2].clone()]),
        ((ids[0].clone(), R::DependsOn.to_relationship_name()), vec![ids[3].clone()]),
    ]);
    let context = build_context_with_saved_holons(saved, relationships);
    let references: Vec<_> = ids
        .iter()
        .map(|id| HolonReference::smart_from_id(context.space_read_handle(), id.clone()))
        .collect();
    assert_eq!(schema_components(&references[0])?, vec![references[1].clone()]);
    assert_eq!(schema_rules(&references[0])?, vec![references[2].clone()]);
    assert_eq!(schema_dependencies(&references[0])?, vec![references[3].clone()]);
    let missing = HolonReference::smart_from_id(
        context.space_read_handle(),
        HolonId::Local(LocalId(vec![99; 39])),
    );
    assert!(schema_components(&missing).is_err());
    assert!(resolve_schema_ownership(&missing, SchemaOwnershipKind::Rule).is_err());
    assert!(ExtendsLineageDiagnosis::assess(&missing).is_err());
    Ok(())
}

#[test]
fn nearest_anchor_wins_and_foreign_transaction_is_rejected() -> Result<(), HolonError> {
    let context = build_context();
    let roots = kind_fixture(&context)?;
    let mut nearer = new_kind_descriptor(&context, "nearer", Some(&roots.holon_type), true)?;
    describe(&mut nearer, &roots.meta_holon_type)?;
    let nearer: HolonReference = nearer.into();
    let child: HolonReference =
        new_kind_descriptor(&context, "child", Some(&nearer), false)?.into();
    assert_eq!(kind_of(&roots, &child)?, Some(nearer.clone()));
    assert!(!TypeHeader::new(&nearer).is_abstract_type()?);
    let mut abstract_anchor =
        new_kind_descriptor(&context, "abstract-anchor", Some(&roots.holon_type), true)?;
    abstract_anchor.with_property_value(P::IsAbstractType, true)?;
    assert!(TypeHeader::new(&abstract_anchor.into()).is_abstract_type()?);
    let foreign_context = build_context();
    let foreign: HolonReference = new_test_holon(&foreign_context, "foreign")?.into();
    // A valid foreign lineage still cannot be consumed by this transaction's semantics.
    let foreign_diagnosis = ExtendsLineageDiagnosis::assess(&foreign)?;
    assert!(matches!(
        roots.semantics.instance_type_kind(foreign_diagnosis.valid_lineage().unwrap()),
        Err(HolonError::CrossTransactionReference { .. })
    ));
    let prerequisite = StructuralPrerequisites::assess(&roots.meta_holon_type)?;
    assert_eq!(
        prerequisite.subject_lineage.lineage,
        prerequisite.governing_lineage.unwrap().lineage
    );
    Ok(())
}

#[test]
fn ownership_rejects_wrong_categories_ambiguous_describers_and_cycles() -> Result<(), HolonError> {
    let context = build_context();
    let schema_type: HolonReference = new_test_holon(&context, "Schema.HolonType")?.into();
    let mut unrelated_type = new_test_holon(&context, "unrelated-type")?;
    let mut owner = new_schema_holon(&context, "owner", &unrelated_type.clone().into())?;
    for kind in [SchemaOwnershipKind::Component, SchemaOwnershipKind::Rule] {
        let mut subject = new_test_holon(&context, "subject")?;
        own(&mut subject, &owner.clone().into(), kind)?;
        assert!(matches!(
            resolve_schema_ownership(&subject.into(), kind)?,
            SchemaOwnershipResolution::InvalidOwner { .. }
        ));
    }
    unrelated_type.add_related_holons(R::Extends, vec![unrelated_type.clone().into()])?;
    let mut subject = new_test_holon(&context, "cyclic-owner")?;
    own(&mut subject, &owner.clone().into(), SchemaOwnershipKind::Rule)?;
    assert!(matches!(
        resolve_schema_ownership(&subject.clone().into(), SchemaOwnershipKind::Rule)?,
        SchemaOwnershipResolution::MalformedOwnerLineage { .. }
    ));
    describe(&mut owner, &schema_type)?;
    assert!(matches!(
        resolve_schema_ownership(&subject.into(), SchemaOwnershipKind::Rule)?,
        SchemaOwnershipResolution::InvalidOwner {
            describing: DescribingTypeResolution::Multiple(_),
            ..
        }
    ));
    Ok(())
}

#[test]
fn staged_ownership_and_local_dependencies_need_no_materialized_inverses() -> Result<(), HolonError>
{
    let context = build_context();
    let schema_type: HolonReference =
        context.mutation().stage_new_holon(new_test_holon(&context, "Schema.HolonType")?)?.into();
    let owner: HolonReference = context
        .mutation()
        .stage_new_holon(new_schema_holon(&context, "owner", &schema_type)?)?
        .into();
    let mut component = new_test_holon(&context, "component")?;
    own(&mut component, &owner, SchemaOwnershipKind::Component)?;
    let component: HolonReference = context.mutation().stage_new_holon(component)?.into();
    assert!(
        matches!(resolve_schema_ownership(&component, SchemaOwnershipKind::Component)?, SchemaOwnershipResolution::OwnedBy(found) if found == owner)
    );
    assert!(schema_components(&owner)?.is_empty());
    let mut parent = new_test_holon(&context, "parent")?;
    parent.add_related_holons(R::DependsOn, vec![owner])?;
    let mut child = new_test_holon(&context, "child")?;
    child.add_related_holons(R::Extends, vec![parent.into()])?;
    assert!(schema_dependencies(&child.into())?.is_empty());
    Ok(())
}

#[test]
fn saved_lineage_recognizes_no_op_staged_root() -> Result<(), HolonError> {
    use crate::core_shared_objects::holon::SavedHolon;
    use base_types::{BaseValue, MapInteger, MapString};
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::{ToPropertyName, ToRelationshipName};
    let root_id = HolonId::Local(LocalId(vec![1; 39]));
    let child_id = HolonId::Local(LocalId(vec![2; 39]));
    let saved = ["TypeDescriptor", "Child"]
        .iter()
        .enumerate()
        .map(|(i, key)| {
            SavedHolon::new(
                LocalId(vec![i as u8 + 1; 39]),
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
            (child_id.clone(), R::Extends.to_relationship_name()),
            vec![root_id.clone()],
        )]),
    );
    let saved_root = crate::SmartReference::new_from_id(context.space_read_handle(), root_id);
    let child = HolonReference::smart_from_id(context.space_read_handle(), child_id);
    let replacement: HolonReference = stage_update_snapshot(
        &context,
        saved_root.holon_id().local_id().clone(),
        new_test_holon(&context, "TypeDescriptor")?,
    )?
    .into();
    let kinds = DescriptorKindSemantics::new(&context);
    // A no-op staged handle represents the saved definition; recognizing it does not substitute content.
    assert!(!same_definition(&child, &replacement)?);
    assert!(equals_or_extends(&saved_root.clone().into(), &replacement)?);
    assert!(ExtendsLineageDiagnosis::assess(&child)?.defects.is_empty());
    let diagnosis = ExtendsLineageDiagnosis::assess(&child)?;
    assert!(kinds.is_descriptor(diagnosis.valid_lineage().unwrap())?);
    let HolonReference::Staged(mut staged) = replacement else { panic!("staged update") };
    staged.with_property_value(P::Description, "new definition")?;
    let successor = staged.into();
    assert!(!equals_or_extends(&child, &successor)?);
    assert!(ExtendsLineageDiagnosis::assess(&child)?.defects.is_empty());
    Ok(())
}

#[test]
fn kind_designation_diagnosis_retains_malformed_value_causes() -> Result<(), HolonError> {
    let context = build_context();
    let roots = kind_fixture(&context)?;
    let mut subject = new_test_holon(&context, "missing-local-designation")?;
    subject.add_related_holons(R::Extends, vec![roots.holon_type.clone()])?;
    describe(&mut subject, &roots.meta_holon_type)?;
    let reference: HolonReference = subject.clone().into();
    let prerequisites = StructuralPrerequisites::assess(&reference)?;
    let lineage = prerequisites.subject_lineage.valid_lineage().unwrap();
    for wrong_type in [false, true] {
        if wrong_type {
            subject.with_property_value(P::DefinesInstanceTypeKind, "false")?;
        }
        for result in [
            roots.semantics.assess_instance_type_kind(lineage).map(|_| ()),
            roots.semantics.assess_describing_lineages_compatible(&prerequisites).map(|_| ()),
        ] {
            let Err(KindResolutionError::InvalidDesignation { descriptor, error }) = result else {
                panic!("readable malformed designation must retain its cause");
            };
            assert!(same_definition(&descriptor, &reference)?);
            assert!(match error {
                HolonError::EmptyField(_) => !wrong_type,
                HolonError::UnexpectedValueType(..) => wrong_type,
                _ => false,
            });
        }
    }

    Ok(())
}

#[test]
fn effective_constraint_contributions_preserve_parent_identity_and_provenance(
) -> Result<(), HolonError> {
    let context = build_context();
    let root: HolonReference = new_kind_descriptor(&context, "TypeDescriptor", None, false)?.into();
    let mut parent = new_kind_descriptor(&context, "parent", Some(&root), true)?;
    let mut child = new_kind_descriptor(&context, "child", Some(&parent.clone().into()), false)?;
    let inherited: HolonReference = new_test_holon(&context, "inherited")?.into();
    let broader: HolonReference = new_test_holon(&context, "broader")?.into();
    parent.add_related_holons(R::Constraints, vec![inherited.clone()])?;
    child.add_related_holons(R::Constraints, vec![broader])?;
    let subject: HolonReference = child.into();
    let diagnosis = ExtendsLineageDiagnosis::assess(&subject)?;
    let mut contributions = ConstraintContributions::resolve(diagnosis.valid_lineage().unwrap())?;
    assert_eq!(contributions.inherited.len(), 1);
    assert_eq!(contributions.effective.len(), 2);
    assert!(contributions.missing_inherited()?.is_empty());
    assert!(same_definition(&contributions.inherited[0].member, &inherited)?);
    assert!(same_definition(&contributions.inherited[0].declared_on, &parent.into())?);
    // Explicit malformed product bypasses additive authoring: replacing provenance
    // is as invalid as removing the inherited identity entirely.
    contributions.effective[0].declared_on = subject;
    assert_eq!(contributions.missing_inherited()?.len(), 1);
    contributions.effective.clear();
    assert_eq!(contributions.missing_inherited()?.len(), 1);
    Ok(())
}

#[test]
fn own_role_and_lineage_roles_preserve_exact_definition_membership() -> Result<(), HolonError> {
    let context = build_context();
    let root: HolonReference = new_kind_descriptor(&context, "TypeDescriptor", None, false)?.into();
    let other_root: HolonReference =
        new_kind_descriptor(&context, "TypeDescriptor", None, false)?.into();
    let child: HolonReference =
        new_kind_descriptor(&context, "PropertyType.TypeDescriptor", Some(&root), false)?.into();
    let diagnosis = ExtendsLineageDiagnosis::assess(&child)?;
    let lineage = diagnosis.valid_lineage().unwrap();
    assert!(has_role(&root, CoreDefinitionRole::TypeDescriptor)?);
    assert!(!has_role(&child, CoreDefinitionRole::TypeDescriptor)?);
    assert!(!lineage.subject_has_role(CoreDefinitionRole::TypeDescriptor));
    assert!(lineage.subject_has_role(CoreDefinitionRole::PropertyType));
    assert!(extends_role(&child, CoreDefinitionRole::TypeDescriptor)?);
    assert!(lineage.contains_role(CoreDefinitionRole::TypeDescriptor));
    assert_eq!(lineage.member_with_role(CoreDefinitionRole::TypeDescriptor), Some(&root));
    assert!(!lineage.contains(&other_root)?);
    assert!(lineage.contains_role_from(&child, CoreDefinitionRole::TypeDescriptor)?);
    assert!(!lineage.contains_role_from(&other_root, CoreDefinitionRole::TypeDescriptor)?);
    assert!(lineage.contains_role(CoreDefinitionRole::PropertyType));
    assert!(!lineage.contains_role_from(&root, CoreDefinitionRole::PropertyType)?);
    assert!(!same_definition(&root, &other_root)?);
    Ok(())
}

#[test]
fn core_roles_read_saved_record_keys_instead_of_smart_properties() -> Result<(), HolonError> {
    use crate::core_shared_objects::holon::SavedHolon;
    use base_types::{BaseValue, MapInteger, MapString};
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::ToPropertyName;

    let saved = ["TypeDescriptor", "Ordinary"]
        .iter()
        .enumerate()
        .map(|(index, key)| {
            SavedHolon::new(
                LocalId(vec![index as u8 + 1; 39]),
                PropertyMap::from([(
                    "Key".to_property_name(),
                    BaseValue::StringValue(MapString((*key).into())),
                )]),
                None,
                MapInteger(1),
            )
        })
        .collect();
    let context = build_context_with_saved_holons(saved, HashMap::new());
    for (index, cached_key, expected_role) in [(1, "Ordinary", true), (2, "TypeDescriptor", false)]
    {
        let subject = HolonReference::smart_with_key(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![index; 39])),
            MapString(cached_key.into()),
        );
        assert_eq!(subject.key()?, Some(MapString(cached_key.into())));
        assert_eq!(has_role(&subject, CoreDefinitionRole::TypeDescriptor)?, expected_role);
        assert_eq!(extends_role(&subject, CoreDefinitionRole::TypeDescriptor)?, expected_role);
        let diagnosis = ExtendsLineageDiagnosis::assess(&subject)?;
        let lineage = diagnosis.valid_lineage().unwrap();
        assert_eq!(lineage.contains_role(CoreDefinitionRole::TypeDescriptor), expected_role);
        assert_eq!(lineage.subject_has_role(CoreDefinitionRole::TypeDescriptor), expected_role);
        assert_eq!(
            lineage.member_with_role(CoreDefinitionRole::TypeDescriptor).is_some(),
            expected_role
        );
        let mut child = new_test_holon(&context, "child")?;
        child.add_related_holons(R::Extends, vec![subject])?;
        let diagnosis = ExtendsLineageDiagnosis::assess(&child.into())?;
        assert_eq!(diagnosis.defects.is_empty(), expected_role);
        if !expected_role {
            assert!(matches!(
                diagnosis.defects.as_slice(),
                [ExtendsLineageDefect::WrongTermination { .. }]
            ));
        }
    }
    Ok(())
}

#[test]
fn role_walk_preserves_short_circuiting_and_structural_errors() -> Result<(), HolonError> {
    let context = build_context();
    let mut root = new_test_holon(&context, "TypeDescriptor")?;
    let parent: HolonReference = new_test_holon(&context, "parent")?.into();
    let other_parent: HolonReference = new_test_holon(&context, "other-parent")?.into();
    root.add_related_holons(R::Extends, vec![parent, other_parent])?;
    let root: HolonReference = root.into();
    assert!(extends_role(&root, CoreDefinitionRole::TypeDescriptor)?);
    assert!(matches!(
        extends_role(&root, CoreDefinitionRole::Schema),
        Err(HolonError::MultipleExtends { .. })
    ));
    let diagnosis = ExtendsLineageDiagnosis::assess(&root)?;
    assert!(diagnosis
        .defects
        .iter()
        .any(|defect| matches!(defect, ExtendsLineageDefect::RootHasParent)));
    assert!(diagnosis.valid_lineage().is_none());
    Ok(())
}

#[test]
fn multiple_type_descriptor_roles_report_the_root_parent_defect_once() -> Result<(), HolonError> {
    let context = build_context();
    let terminal: HolonReference = new_test_holon(&context, "TypeDescriptor")?.into();
    let mut middle = new_test_holon(&context, "TypeDescriptor")?;
    middle.add_related_holons(R::Extends, vec![terminal])?;
    let mut subject = new_test_holon(&context, "TypeDescriptor")?;
    subject.add_related_holons(R::Extends, vec![middle.into()])?;
    let diagnosis = ExtendsLineageDiagnosis::assess(&subject.into())?;
    assert_eq!(diagnosis.lineage.len(), 3);
    assert!(matches!(diagnosis.defects.as_slice(), [ExtendsLineageDefect::RootHasParent]));
    Ok(())
}
