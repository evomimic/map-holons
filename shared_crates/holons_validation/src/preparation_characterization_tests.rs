//! Baseline behavior controls for preparation and readiness narrowing.
//!
//! The two ignored evidence tests are temporary. Run them explicitly with `--ignored`
//! for the baseline and again after Step C. In the final verification step, delete them and
//! `backend_membership_inventory` when the generic zero-query test replaces them;
//! retain the exact-source `reads_by_source` counter. Kept scenario tests change
//! assertions in Step A without being renamed.
use super::*;
use crate::commit_assessment::{assess_commit_candidates, validate_commit_candidates};
use holons_core::core_shared_objects::holon::ValidationState;
use type_names::CoreRelationshipTypeName as R;

#[test]
#[ignore = "temporary baseline evidence; rerun after Step C"]
fn broad_preparation_incidental_empty_reads_abort_and_preserve_outcomes() -> Result<(), HolonError>
{
    // These memberships have no semantic consumer on these categories. A retained
    // required edge is included as a paired control, not a proposed removal.
    for (source, edge) in [
        ("Contract", R::ValueType),
        ("StringValueType.ValueType", R::InstanceProperties),
        ("StringValueType.ValueType", R::RuleOf),
        ("Title.PropertyType", R::DescribedBy),
    ] {
        let fixture = readiness_fixture()?.saved_snapshot()?;
        fixture.control_saved_relationship_reads(source, edge.clone(), None)?;
        let mut subject = fixture.staged_subject("read-control")?;
        subject.with_property_value("Title", "valid")?;
        assert!(validate_commit_candidates(&fixture.context, &[subject.clone()])?.is_accepted());
        assert!(fixture.backend_reads_for_source(source, &edge) > 0, "{source} {edge:?}");
        let before = fixture.backend_reads_for_source(source, &edge);
        let error = HolonError::InvalidState(format!("injected {source} {edge:?}"));
        fixture.control_saved_relationship_reads(source, edge.clone(), Some(error.clone()))?;
        for report_only in [true, false] {
            let result = if report_only {
                assess_commit_candidates(&fixture.context, &[subject.clone()]).map(|a| a.report)
            } else {
                validate_commit_candidates(&fixture.context, &[subject.clone()])
            };
            assert_eq!(result.unwrap_err(), error);
            assert_eq!(subject.validation_state()?, ValidationState::Validated);
            assert!(subject.validation_findings()?.is_empty());
        }
        assert!(fixture.backend_reads_for_source(source, &edge) >= before + 2);
    }
    Ok(())
}

#[test]
#[ignore = "temporary baseline evidence; rerun after Step C"]
fn exact_empty_membership_reads_cover_each_candidate_mode() -> Result<(), HolonError> {
    for mode in ["instance", "definition", "mixed", "no-op", "graph-only"] {
        let fixture = saved_instance_fixture()?;
        let mut instance = fixture.staged_subject("inventory-instance")?;
        instance.with_property_value("Title", "valid")?;
        let definition = fixture.stage_update_of_with("Title.PropertyType", |model| {
            use type_names::ToPropertyName;
            model.properties.insert(
                "IsValueRequired".to_property_name(),
                BaseValue::BooleanValue(MapBoolean(false)),
            );
            Ok(())
        })?;
        let alias = fixture.stage_update_of("SavedInstance")?;
        if mode == "graph-only" {
            let handle = alias.get_holon_to_commit(&fixture.context)?;
            let mut holon = handle.write().unwrap();
            let Holon::Staged(staged) = &mut *holon else { unreachable!() };
            staged.note_relationship_mutation(false)?;
        }
        let candidates = match mode {
            "instance" => vec![instance],
            "definition" => vec![definition],
            "mixed" => vec![definition, instance],
            _ => vec![alias],
        };
        fixture.control_saved_relationship_reads("StringValueType.ValueType", R::RuleOf, None)?;
        fixture.reset_backend_membership_reads();
        let report = assess_commit_candidates(&fixture.context, &candidates)?.report;
        assert_eq!(
            report.is_accepted(),
            !matches!(mode, "definition" | "mixed"),
            "{mode}: {report:?}"
        );
        assert!(
            fixture.backend_reads_for_source("StringValueType.ValueType", &R::RuleOf) > 0,
            "{mode}"
        );
        for (source, relationship, count) in fixture.backend_membership_inventory() {
            println!("[fixture-membership] mode={mode} source={source} relationship={relationship} requests={count}");
        }
    }
    Ok(())
}

#[test]
fn unusable_property_selection_beside_invalid_sibling_value() -> Result<(), HolonError> {
    let usable = readiness_fixture()?;
    let mut control = usable.staged_subject("usable-control")?;
    control.with_property_value("Key", 42_i64)?;
    let control_report = validate_commit_candidates(&usable.context, &[control])?;
    assert!(has_rule_code(&control_report.violations, "BaseValueKindMismatch"));
    assert!(has_rule_code(&control_report.violations, "DS-PROP-001"));
    let mut fixture = readiness_fixture()?;
    fixture.link("Title.PropertyType", R::ValueType, "BooleanValueType.ValueType")?;
    let fixture = fixture.saved_snapshot()?;
    let mut subject = fixture.staged_subject("unusable-property")?;
    subject.with_property_value("Key", 42_i64)?;
    let report = validate_commit_candidates(&fixture.context, &[subject.clone()])?;
    assert!(!report.is_accepted());
    assert!(subject
        .validation_findings()?
        .iter()
        .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    assert!(!has_rule_code(&subject.validation_findings()?, "BaseValueKindMismatch"));
    assert!(!has_rule_code(&subject.validation_findings()?, "DS-PROP-001"));
    assert_eq!(
        report
            .unattached_findings()?
            .iter()
            .filter(|f| f
                .message
                .contains("Saved governing contract has invalid effective member definitions"))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn unrelated_staged_ownership_defect_beside_its_consumer() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    prepare_meta_contract(&mut fixture)?;
    fixture.link("Contract", R::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
    let contract = fixture.nodes["Contract"].clone();
    let HolonReference::Staged(definition) = contract.clone() else { unreachable!() };
    let mut input = fixture.context.mutation().new_holon(Some("ownership-consumer".into()))?;
    input.add_related_holons(R::DescribedBy, vec![contract])?;
    let subject = fixture.context.mutation().stage_new_holon(input)?;
    for candidates in [[subject.clone(), definition.clone()], [definition.clone(), subject.clone()]]
    {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(
            definition.validation_findings()?.len(),
            1,
            "only the ownership defect is relevant"
        );
        assert!(has_rule_code(&definition.validation_findings()?, "SchemaOwnership"));
        assert!(subject
            .validation_findings()?
            .iter()
            .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    }
    Ok(())
}

#[test]
fn malformed_governing_lineage_with_multiple_consumers() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.link("Contract", R::Extends, "PropertyType.TypeDescriptor")?;
    let fixture = fixture.saved_snapshot()?;
    let make = |key: &str| -> Result<_, HolonError> {
        use holons_core::core_shared_objects::WriteableHolonState;
        let input = fixture.context.mutation().new_holon(Some(key.into()))?;
        let subject = fixture.context.mutation().stage_new_holon(input)?;
        let handle = subject.get_holon_to_commit(&fixture.context)?;
        let mut holon = handle.write().unwrap();
        let Holon::Staged(staged) = &mut *holon else { unreachable!() };
        staged.add_related_holons_with_keys(
            R::DescribedBy.as_relationship_name(),
            vec![(fixture.nodes["Contract"].clone(), None)],
        )?;
        Ok(subject)
    };
    let first = make("bad-governor-first")?;
    let second = make("bad-governor-second")?;
    for candidates in [[first.clone(), second.clone()], [second.clone(), first.clone()]] {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        for subject in &candidates {
            assert_eq!(subject.validation_state()?, ValidationState::Invalid);
            assert!(subject
                .validation_findings()?
                .iter()
                .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
        }
    }
    Ok(())
}

#[test]
fn malformed_candidate_lineages() -> Result<(), HolonError> {
    for cycle in [true, false] {
        let mut fixture = readiness_fixture()?;
        let rule = if cycle {
            CoreValidationRuleName::AcyclicExtendsLineage
        } else {
            CoreValidationRuleName::AtMostOneDirectParent
        };
        fixture.link("MetaHolonType.MetaTypeDescriptor", R::ValidationBindings, rule.as_str())?;
        fixture.node("Malformed")?;
        fixture.link(
            "Malformed",
            R::Extends,
            if cycle { "Malformed" } else { "HolonType.TypeDescriptor" },
        )?;
        if !cycle {
            fixture.link("Malformed", R::Extends, "PropertyType.TypeDescriptor")?;
        }
        fixture.link("Malformed", R::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
        let HolonReference::Staged(subject) = fixture.nodes["Malformed"].clone() else {
            unreachable!()
        };
        let report = validate_commit_candidates(&fixture.context, &[subject.clone()])?;
        assert!(!report.is_accepted());
        assert_eq!(subject.validation_state()?, ValidationState::Invalid);
        assert!(has_rule_code(
            &subject.validation_findings()?,
            if cycle { "DS-STRUCT-003" } else { "DS-STRUCT-002" }
        ));
    }
    Ok(())
}

#[test]
fn mutual_description_with_later_peer_failure() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    prepare_meta_contract(&mut fixture)?;
    fixture.node("PeerOwner")?;
    fixture.link("PeerOwner", R::DescribedBy, "Schema.HolonType")?;
    for name in ["PeerA", "PeerB"] {
        fixture.node(name)?.with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(name, R::Extends, "MetaHolonType.MetaTypeDescriptor")?;
        fixture.link(name, R::ComponentOf, "PeerOwner")?;
    }
    fixture.link("PeerA", R::InstanceProperties, "Title.PropertyType")?;
    fixture.link("PeerA", R::DescribedBy, "PeerB")?;
    fixture.link("PeerB", R::DescribedBy, "PeerA")?;
    let HolonReference::Staged(a) = fixture.nodes["PeerA"].clone() else { unreachable!() };
    let HolonReference::Staged(b) = fixture.nodes["PeerB"].clone() else { unreachable!() };
    for subject in [a.clone(), b.clone()] {
        let reference = subject.into();
        let pre = holons_core::StructuralPrerequisites::assess(&reference)?;
        let mut prod = DescriptorRuleProducts::default();
        prod.prepare_structure(&pre);
        prod.prepare_kind(&pre, &holons_core::DescriptorKindSemantics::new(&fixture.context))?;
        prod.prepare_effective_schema(
            &holons_core::descriptors::ComputedEffectiveSchema::resolve(&reference)?,
            &holons_core::DescriptorKindSemantics::new(&fixture.context),
        )?;
        assert!(!prod.has_findings(), "mutual peer product must remain usable");
    }
    for candidates in [[a.clone(), b.clone()], [b.clone(), a.clone()]] {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(report.violations.len(), 2, "one peer failure and one propagated blocker");
        assert_eq!(a.validation_findings()?.len(), 1);
        assert_eq!(b.validation_findings()?.len(), 1);
        assert!(has_rule_code(&b.validation_findings()?, "DS-PROP-001"));
        assert!(has_rule_code(&report.violations, "DS-PROP-001"));
        assert!(a
            .validation_findings()?
            .iter()
            .any(|f| f.message.contains("did not establish readiness")
                || f.message.contains("Required prospective commitment")));
        assert_eq!(a.validation_state()?, ValidationState::Invalid);
        assert_eq!(b.validation_state()?, ValidationState::Invalid);
    }
    Ok(())
}

/// Supplies a usable meta-contract so readiness findings can be isolated from product defects.
fn prepare_meta_contract(fixture: &mut Fixture) -> Result<(), HolonError> {
    fixture.link("HolonType.TypeDescriptor", R::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
    fixture.link("Schema.HolonType", R::Extends, "HolonType.TypeDescriptor")?;
    for name in ["Extends", "ComponentOf", "InstanceProperties"] {
        let key = format!("Peer{name}.Relationship");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(&key, R::Extends, "DeclaredRelationshipType.RelationshipType")?;
        fixture.link(&key, R::SourceType, "HolonType.TypeDescriptor")?;
        fixture.link(&key, R::TargetType, "HolonType.TypeDescriptor")?;
        fixture.link("MetaHolonType.MetaTypeDescriptor", R::InstanceRelationships, &key)?;
    }
    for name in ["Key", "TypeName", "IsAbstractType", "IsValueRequired", "DefinesInstanceTypeKind"]
    {
        let key = format!("Peer{name}.Property");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(&key, R::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(
            &key,
            R::ValueType,
            if matches!(name, "Key" | "TypeName") {
                "StringValueType.ValueType"
            } else {
                "BooleanValueType.ValueType"
            },
        )?;
        fixture.link("MetaHolonType.MetaTypeDescriptor", R::InstanceProperties, &key)?;
    }
    Ok(())
}

/// Tests semantic rule attribution without coupling callers to the finding representation.
fn has_rule_code(findings: &[core_types::CommitValidationViolation], expected: &str) -> bool {
    findings.iter().any(|finding| {
        matches!(
            &finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == expected
        )
    })
}
