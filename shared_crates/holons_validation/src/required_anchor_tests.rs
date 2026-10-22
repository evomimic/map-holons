use super::*;
use crate::{
    assessment_support::path,
    prospective::{assess_required_anchor_updates, REQUIRED_ANCHOR_KEYS},
};
use base_types::ToBaseValue;
use holons_core::core_shared_objects::holon::StagedState;
use type_names::ToPropertyName;

fn restricted(report: &CommitValidationReport) -> Vec<&core_types::CommitValidationViolation> {
    report.violations.iter().filter(|finding| matches!(&finding.kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "UnsupportedRequiredAnchorUpdate"
    )).collect()
}

/// Completes descriptor conformance inputs so restriction findings are isolated
/// from missing metadata declarations, describing types, or Schema ownership.
pub(super) fn described_anchor_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = readiness_fixture()?;
    for (name, value_type) in [
        ("TypeName", "StringValueType.ValueType"),
        ("IsAbstractType", "BooleanValueType.ValueType"),
        ("IsValueRequired", "BooleanValueType.ValueType"),
        ("DefinesInstanceTypeKind", "BooleanValueType.ValueType"),
    ] {
        let key = format!("{name}.AnchorMetadataProperty");
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
        let key = format!("{name}.AnchorMetadataRelationship");
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
    fixture.node("AnchorOwner")?;
    fixture.link("AnchorOwner", CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    for anchor in
        ["HolonType.TypeDescriptor", "StringValueType.ValueType", "PropertyType.TypeDescriptor"]
    {
        fixture.link(anchor, CoreRelationshipTypeName::ComponentOf, "AnchorOwner")?;
        fixture.link(
            anchor,
            CoreRelationshipTypeName::DescribedBy,
            "MetaHolonType.MetaTypeDescriptor",
        )?;
    }
    fixture.saved_snapshot()
}

#[test]
fn anchor_successors_and_rekeys_reject_without_masking_other_candidates() -> Result<(), HolonError>
{
    for rekey in [false, true] {
        let fixture = described_anchor_fixture()?;
        let update = fixture.stage_update_of_with("HolonType.TypeDescriptor", |model| {
            if rekey {
                model.properties.insert("Key".to_property_name(), "RekeyedRoot".to_base_value());
            }
            Ok(())
        })?;
        let independent = fixture.context.mutation().stage_new_holon(
            fixture
                .context
                .mutation()
                .new_holon(Some(MapString("undescribed-independent".into())))?,
        )?;
        let mut consumer = fixture.staged_subject("saved-anchor-consumer")?;
        consumer.with_property_value("Title", "ready")?;
        let candidates = [update.clone(), independent.clone(), consumer.clone()];
        let assessment = assess_commit_candidates(&fixture.context, &candidates)?;
        assert!(!assessment.report.is_accepted());
        assert_eq!(assessment.report.violation_count(), 2, "{:#?}", assessment.report.violations);
        assert_eq!(restricted(&assessment.report).len(), 1);
        let finding = restricted(&assessment.report)[0];
        assert_eq!(finding.subject, path(&update.clone().into()));
        assert!(finding.rule_key.is_none());
        assert!(finding.message.contains("HolonType.TypeDescriptor"));
        assert!(finding.message.contains("saved source"));
        assert!(!finding.message.contains('#'), "diagnostics name no issue numbers");
        assert!(assessment
            .report
            .violations
            .iter()
            .any(|finding| finding.subject == path(&independent.clone().into())));
        // The profiled production entry and report-only entry share the restriction.
        let reads_before = fixture.lineage_lookup_reads();
        let mut key_evidence_reads = 0;
        let report =
            validate_commit_candidates_with_profile(&fixture.context, &candidates, |event| {
                if matches!(
                    event,
                    crate::AssessmentEvent::PreparationOperation {
                        role: "key-evidence",
                        starting: true,
                        ..
                    }
                ) {
                    key_evidence_reads += 1;
                }
            })?;
        assert_eq!(
            fixture.lineage_lookup_reads() - reads_before,
            key_evidence_reads,
            "direct anchor recognition must add no inventory reads"
        );
        assert_eq!(report, assessment.report);
        assert_eq!(
            consumer.validation_state()?,
            ValidationState::Validated,
            "{:#?}",
            report.violations
        );
        assert!(consumer.validation_findings()?.is_empty());
        assert_eq!(update.validation_state()?, ValidationState::Invalid);
        assert_eq!(update.validation_findings()?.len(), 1, "{:#?}", update.validation_findings()?);
        assert!(update.validation_findings()?.iter().any(
            |finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code }
                if code == "UnsupportedRequiredAnchorUpdate")
        ));
        assert!(update.is_in_state(&fixture.context, StagedState::ForUpdateNewVersion)?);
    }
    Ok(())
}

#[test]
fn previously_rekeyed_non_root_source_is_identified_by_saved_lineage() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("RekeyedAnchorV2")?;
    let fixture =
        fixture.saved_snapshot_with_lineages(&[("RekeyedAnchorV2", "HolonType.TypeDescriptor")])?;
    let successor = fixture.stage_update_of_with("RekeyedAnchorV2", |_| Ok(()))?;
    let mut collector = ValidationCollector::default();
    assess_required_anchor_updates(&fixture.context, &[successor.clone()], &mut collector)?;
    let report = collector.into_report();
    assert_eq!(restricted(&report).len(), 1);
    assert_eq!(report.violations[0].subject, path(&successor.into()));
    assert!(report.violations[0].message.contains("HolonType.TypeDescriptor"));
    assert_eq!(fixture.lineage_lookup_reads(), REQUIRED_ANCHOR_KEYS.len());
    Ok(())
}

#[test]
fn historical_anchor_evidence_is_loaded_once_and_only_when_needed() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("OtherCoreDefinition")?;
    let fixture = fixture.saved_snapshot()?;
    let no_op = fixture.stage_update_of("HolonType.TypeDescriptor")?;
    let graph_only = fixture.stage_update_of("HolonType.TypeDescriptor")?;
    {
        let handle = graph_only.get_holon_to_commit(&fixture.context)?;
        let mut holon = handle.write().unwrap();
        let Holon::Staged(staged) = &mut *holon else { unreachable!() };
        staged.note_relationship_mutation(false)?;
    }
    let created = fixture.staged_subject("ordinary-create")?;
    let mut collector = ValidationCollector::default();
    assess_required_anchor_updates(
        &fixture.context,
        &[no_op, graph_only, created],
        &mut collector,
    )?;
    assert!(collector.violations().is_empty());
    assert_eq!(fixture.lineage_lookup_reads(), 0);
    let first = fixture.stage_update_of_with("OtherCoreDefinition", |_| Ok(()))?;
    let second = fixture.stage_update_of_with("OtherCoreDefinition", |_| Ok(()))?;
    assess_required_anchor_updates(&fixture.context, &[first, second], &mut collector)?;
    assert!(collector.violations().is_empty(), "the bound does not cover every Core definition");
    assert_eq!(fixture.lineage_lookup_reads(), REQUIRED_ANCHOR_KEYS.len());
    Ok(())
}

#[test]
fn anchor_evidence_errors_preserve_prior_outcomes() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let mut subject = fixture.staged_subject("previously-valid")?;
    subject.with_property_value("Title", "valid")?;
    assert!(validate_commit_candidates(&fixture.context, &[subject.clone()])?.is_accepted());
    let previous = subject.validation_state()?;
    let update = fixture.stage_update_of_with("Contract", |_| Ok(()))?;
    fixture.fail_lineage_lookup(HolonError::InvalidState("injected key evidence failure".into()));
    assert!(matches!(validate_commit_candidates(&fixture.context, &[subject.clone(), update]),
        Err(HolonError::InvalidState(message)) if message == "injected key evidence failure"));
    assert_eq!(subject.validation_state()?, previous);
    assert!(subject.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn anchor_inventory_is_explicit_and_excludes_inactive_schema_rule() {
    let unique: std::collections::HashSet<_> = REQUIRED_ANCHOR_KEYS.iter().collect();
    assert_eq!(unique.len(), REQUIRED_ANCHOR_KEYS.len());
    assert!(REQUIRED_ANCHOR_KEYS.contains(&"KeyUniqueness.ValidationRule"));
    assert!(!REQUIRED_ANCHOR_KEYS.contains(&"CrossSchemaDependenciesDeclared.ValidationRule"));
}

#[test]
fn explicit_anchor_successor_dependency_does_not_establish_readiness() -> Result<(), HolonError> {
    let mut fixture = described_anchor_fixture()?;
    let successor = fixture.stage_update_of_with("StringValueType.ValueType", |_| Ok(()))?;
    let mut properties = Vec::new();
    for (key, value_type) in [
        ("ExplicitAnchorConsumer", successor.clone().into()),
        ("SavedAnchorConsumer", fixture.nodes["StringValueType.ValueType"].clone()),
    ] {
        let mut property = fixture.node(key)?;
        property.with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(key, CoreRelationshipTypeName::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(key, CoreRelationshipTypeName::ComponentOf, "AnchorOwner")?;
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
        assert_eq!(restricted(&report).len(), 1);
        assert_eq!(
            successor.validation_findings()?.len(),
            1,
            "{:#?}",
            successor.validation_findings()?
        );
        let findings = properties[0].validation_findings()?;
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].kind, CommitValidationViolationKind::UnresolvedLocalDependency);
        assert!(findings[0].message.contains(&successor.reference_id_string()));
        assert_eq!(properties[0].validation_state()?, ValidationState::Invalid);
        assert_eq!(properties[1].validation_state()?, ValidationState::Validated);
        assert!(properties[1].validation_findings()?.is_empty());
    }
    Ok(())
}
