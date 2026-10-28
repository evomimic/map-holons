use super::*;
use base_types::ToBaseValue;
use type_names::ToPropertyName;

/// Completes descriptor conformance inputs so successor findings are isolated
/// from missing metadata declarations, describing types, or Schema ownership.
fn core_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = readiness_fixture()?;
    for (name, value_type) in [
        ("TypeName", "StringValueType.ValueType"),
        ("IsAbstractType", "BooleanValueType.ValueType"),
        ("IsValueRequired", "BooleanValueType.ValueType"),
        ("DefinesInstanceTypeKind", "BooleanValueType.ValueType"),
    ] {
        let key = format!("{name}.CoreMetadataProperty");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(&key, CoreRelationshipTypeName::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(&key, CoreRelationshipTypeName::ValueType, value_type)?;
        fixture.link(
            "MetaTypeDescriptor.HolonType",
            CoreRelationshipTypeName::InstanceProperties,
            &key,
        )?;
    }
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::InstanceProperties,
        "Key.PropertyType",
    )?;
    for (name, target) in [
        ("Extends", "TypeDescriptor"),
        ("ComponentOf", "Schema.HolonType"),
        ("InstanceProperties", "PropertyType.TypeDescriptor"),
        ("InstanceRelationships", "DeclaredRelationshipType.RelationshipType"),
        ("ValidationBindings", "Rule.HolonType"),
        ("Predecessor", "HolonType.TypeDescriptor"),
        ("ValueType", "ValueType.TypeDescriptor"),
    ] {
        let key = format!("{name}.CoreMetadataRelationship");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(
            &key,
            CoreRelationshipTypeName::Extends,
            "DeclaredRelationshipType.RelationshipType",
        )?;
        fixture.link(&key, CoreRelationshipTypeName::SourceType, "MetaTypeDescriptor.HolonType")?;
        fixture.link(&key, CoreRelationshipTypeName::TargetType, target)?;
        fixture.link(
            "MetaTypeDescriptor.HolonType",
            CoreRelationshipTypeName::InstanceRelationships,
            &key,
        )?;
    }
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.node("CoreOwner")?;
    fixture.link("CoreOwner", CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    for core_definition in
        ["HolonType.TypeDescriptor", "StringValueType.ValueType", "PropertyType.TypeDescriptor"]
    {
        fixture.link(core_definition, CoreRelationshipTypeName::ComponentOf, "CoreOwner")?;
        fixture.link(
            core_definition,
            CoreRelationshipTypeName::DescribedBy,
            "MetaHolonType.MetaTypeDescriptor",
        )?;
    }
    Ok(fixture)
}

pub(super) fn described_core_fixture() -> Result<Fixture, HolonError> {
    core_fixture()?.saved_snapshot()
}

#[test]
fn core_successors_and_rekeys_preserve_independent_findings_and_retry() -> Result<(), HolonError> {
    for rekey in [false, true] {
        let fixture = described_core_fixture()?;
        let update = fixture.stage_update_of_with("HolonType.TypeDescriptor", |model| {
            model
                .properties
                .insert("TypeName".to_property_name(), "HolonSuccessor".to_base_value());
            if rekey {
                model.properties.insert("Key".to_property_name(), "RekeyedRoot".to_base_value());
            }
            Ok(())
        })?;
        let mut independent = fixture.staged_subject("independent-missing-title")?;
        let candidates = [update.clone(), independent.clone()];
        let assessment = assess_commit_candidates(&fixture.context, &candidates)?;
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert_eq!(report, assessment.report);
        assert_eq!(report.violation_count(), 1, "{report:#?}");
        assert_eq!(independent.validation_findings()?.len(), 1);
        assert!(matches!(&independent.validation_findings()?[0].kind,
            CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001"));
        assert_eq!(update.validation_state()?, ValidationState::Validated);
        assert!(update.validation_findings()?.is_empty());
        independent.with_property_value("Title", "corrected")?;
        let mut anchor_events = 0;
        let report =
            validate_commit_candidates_with_profile(&fixture.context, &candidates, |event| {
                if matches!(
                    event,
                    crate::AssessmentEvent::PreparationOperation { role: "anchor-evidence", .. }
                ) {
                    anchor_events += 1;
                }
            })?;
        assert!(report.is_accepted(), "{report:#?}");
        assert_eq!(anchor_events, 0);
        assert_eq!(independent.validation_state()?, ValidationState::Validated);
        assert!(independent.validation_findings()?.is_empty());
    }
    Ok(())
}

/// The same role and dispatch keys never combine the two definitions' contracts or findings.
#[test]
fn exact_core_predecessor_and_successor_consumers_keep_separate_contracts() -> Result<(), HolonError>
{
    use holons_core::{HolonCollection, HolonCollectionApi};
    use std::sync::{Arc, RwLock};
    let fixture = described_core_fixture()?;
    let successor = fixture.stage_update_of_with("HolonType.TypeDescriptor", |model| {
        let mut members = HolonCollection::new_transient();
        members.add_references(vec![fixture.nodes["Title.PropertyType"].clone()])?;
        model.relationships.as_mut().unwrap().insert(
            CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
            Arc::new(RwLock::new(members)),
        );
        Ok(())
    })?;
    assert_eq!(successor.key()?, fixture.nodes["HolonType.TypeDescriptor"].key()?);
    assert!(!holons_core::same_definition(
        &successor.clone().into(),
        &fixture.nodes["HolonType.TypeDescriptor"]
    )?);
    let predecessor_contract =
        holons_core::ContractContributions::resolve(&fixture.nodes["HolonType.TypeDescriptor"])?;
    let successor_contract =
        holons_core::ContractContributions::resolve(&successor.clone().into())?;
    assert!(predecessor_contract.properties.is_empty());
    assert_eq!(successor_contract.properties.len(), 1);
    assert!(holons_core::same_definition(
        &successor_contract.properties[0].member,
        &fixture.nodes["Title.PropertyType"]
    )?);
    let mut consumers = Vec::new();
    // This minimal root contract declares no Key property. Keyless consumers
    // isolate the contract difference without undeclared-property findings.
    for (index, descriptor) in
        [fixture.nodes["HolonType.TypeDescriptor"].clone(), successor.clone().into()]
            .into_iter()
            .enumerate()
    {
        let mut input =
            fixture.context.mutation().new_holon(Some(format!("consumer-{index}").into()))?;
        input.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor])?;
        let mut consumer = fixture.context.mutation().stage_new_holon(input)?;
        consumer.remove_property_value("Key")?;
        consumers.push(consumer);
    }
    for reversed in [false, true] {
        let mut candidates = vec![successor.clone(), consumers[0].clone(), consumers[1].clone()];
        if reversed {
            candidates.reverse();
        }
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert_eq!(successor.validation_state()?, ValidationState::Validated, "{report:#?}");
        assert_eq!(consumers[0].validation_state()?, ValidationState::Validated, "{report:#?}");
        assert!(consumers[0].validation_findings()?.is_empty());
        assert_eq!(report.violation_count(), 1, "{report:#?}");
        let findings = consumers[1].validation_findings()?;
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(matches!(&findings[0].kind, CommitValidationViolationKind::RuleViolation { code }
            if code == "DS-PROP-001"));
        assert_eq!(
            findings[0].rule_key.as_ref().unwrap(),
            CoreValidationRuleName::RequiredPropertyPresence.as_str()
        );
    }
    consumers[1].with_property_value("Title", "successor contract satisfied")?;
    let report = validate_commit_candidates(
        &fixture.context,
        &[successor, consumers[0].clone(), consumers[1].clone()],
    )?;
    assert!(report.is_accepted(), "{report:#?}");
    Ok(())
}

#[test]
fn explicit_core_value_successor_and_saved_predecessor_are_both_valid() -> Result<(), HolonError> {
    let mut fixture = described_core_fixture()?;
    let successor = fixture.stage_update_of_with("StringValueType.ValueType", |_| Ok(()))?;
    let mut properties = Vec::new();
    for (key, value_type) in [
        ("ExplicitCoreConsumer", successor.clone().into()),
        ("SavedCoreConsumer", fixture.nodes["StringValueType.ValueType"].clone()),
    ] {
        let mut property = fixture.node(key)?;
        property.with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(key, CoreRelationshipTypeName::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(key, CoreRelationshipTypeName::ComponentOf, "CoreOwner")?;
        property.add_related_holons(CoreRelationshipTypeName::ValueType, vec![value_type])?;
        fixture.link(
            key,
            CoreRelationshipTypeName::DescribedBy,
            "MetaHolonType.MetaTypeDescriptor",
        )?;
        let HolonReference::Staged(property) = property else { unreachable!() };
        properties.push(property);
    }
    for reversed in [false, true] {
        let mut candidates = vec![successor.clone(), properties[0].clone(), properties[1].clone()];
        if reversed {
            candidates.reverse();
        }
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(report.is_accepted(), "{report:#?}");
    }
    Ok(())
}

#[test]
fn required_key_evidence_errors_preserve_prior_outcomes_for_core_successors(
) -> Result<(), HolonError> {
    let fixture = described_core_fixture()?;
    let mut subject = fixture.staged_subject("previously-valid")?;
    subject.with_property_value("Title", "valid")?;
    assert!(validate_commit_candidates(&fixture.context, &[subject.clone()])?.is_accepted());
    let previous = subject.validation_state()?;
    let update = fixture.stage_update_of_with("HolonType.TypeDescriptor", |_| Ok(()))?;
    fixture.fail_lineage_lookup(HolonError::InvalidState("injected key evidence failure".into()));
    assert!(matches!(validate_commit_candidates(&fixture.context, &[subject.clone(), update]),
        Err(HolonError::InvalidState(message)) if message == "injected key evidence failure"));
    assert_eq!(subject.validation_state()?, previous);
    assert!(subject.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn previously_rekeyed_core_successor_needs_only_its_own_key_evidence() -> Result<(), HolonError> {
    let mut fixture = core_fixture()?;
    fixture
        .node("RekeyedCoreV2")?
        .with_property_value("Key", "RekeyedCore")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("RekeyedCoreV2", CoreRelationshipTypeName::Extends, "HolonType.TypeDescriptor")?;
    fixture.link("RekeyedCoreV2", CoreRelationshipTypeName::ComponentOf, "CoreOwner")?;
    fixture.link(
        "RekeyedCoreV2",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let fixture =
        fixture.saved_snapshot_with_lineages(&[("RekeyedCoreV2", "HolonType.TypeDescriptor")])?;
    let successor = fixture.stage_update_of_with("RekeyedCoreV2", |_| Ok(()))?;
    let reads_before = fixture.lineage_lookup_reads();
    let mut evidence_loads = 0;
    let report =
        validate_commit_candidates_with_profile(&fixture.context, &[successor], |event| {
            if matches!(
                event,
                crate::AssessmentEvent::PreparationOperation {
                    role: "key-evidence",
                    starting: true,
                    ..
                }
            ) {
                evidence_loads += 1;
            }
        })?;
    assert!(report.is_accepted(), "{report:#?}");
    assert_eq!(evidence_loads, 1);
    assert_eq!(
        fixture.lineage_lookup_reads() - reads_before,
        evidence_loads,
        "assessment reads candidate key evidence without an anchor-inventory scan"
    );
    Ok(())
}
