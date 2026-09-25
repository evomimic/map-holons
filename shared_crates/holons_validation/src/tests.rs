#[path = "constraint_declarations_tests.rs"]
mod constraint_declarations;

#[path = "test_support.rs"]
mod fixture;

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::core_shared_objects::{holon::ValidationState, Holon};
use holons_core::{Descriptor, HolonReference, PropertyDescriptor, ValueDescriptor, WritableHolon};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

use super::*;
use crate::orchestration::subject_gate_tests::validate_subject_candidates;
use fixture::{Fixture, RULES};

fn property_path(name: &str) -> ValidationSubjectPath {
    ValidationSubjectPath::Property { holon_identity: "subject".into(), name: name.into() }
}

fn value_path() -> ValidationSubjectPath {
    ValidationSubjectPath::Value { holon_identity: "subject".into(), property: "Title".into() }
}

#[test]
fn subject_gate_replaces_all_outcomes_and_accepts_corrected_retry() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let mut missing_title = fixture.staged_subject("missing-title")?;
    let mut clean = fixture.staged_subject("clean")?;
    clean.with_property_value("Title", "present")?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("undescribed".into())))?;
    let mut undescribed = fixture.context.mutation().stage_new_holon(transient)?;

    // Seed outcomes contrary to the authored inputs: no prior state may skip reassessment.
    missing_title.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    let stale_findings =
        validate_subject_candidates(&fixture.context, std::slice::from_ref(&undescribed))?
            .violations;
    clean.replace_validation_outcome(ValidationState::Invalid, stale_findings)?;
    undescribed.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    {
        let rc_holon = missing_title.get_holon_to_commit(&fixture.context)?;
        let mut holon = rc_holon.write().expect("test staged holon lock");
        let Holon::Staged(staged) = &mut *holon else {
            unreachable!("staged reference must resolve to a staged holon");
        };
        staged.add_error(HolonError::NotImplemented("prior persistence failure".into()))?;
    }

    let candidates = [missing_title.clone(), clean.clone(), undescribed.clone()];
    let report = validate_subject_candidates(&fixture.context, &candidates)?;
    assert!(!report.is_accepted());
    assert_eq!(report.violation_count(), 2);
    assert_eq!(missing_title.validation_state()?, ValidationState::Invalid);
    assert_eq!(clean.validation_state()?, ValidationState::Validated);
    assert!(clean.validation_findings()?.is_empty());
    assert_eq!(undescribed.validation_state()?, ValidationState::NoDescriptor);
    assert_eq!(
        report.violations,
        [missing_title.validation_findings()?, undescribed.validation_findings()?].concat()
    );
    assert!(matches!(
        &report.violations[0].kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001"
    ));
    assert_eq!(
        report.violations[0].rule_key.as_deref(),
        Some(CoreValidationRuleName::RequiredPropertyPresence.as_str())
    );
    assert_eq!(report.violations[1].kind, CommitValidationViolationKind::NoDescriptor);
    assert_eq!(
        missing_title.commit_errors()?,
        vec![HolonError::NotImplemented("prior persistence failure".into())]
    );

    missing_title.with_property_value("Title", "corrected")?;
    undescribed.with_descriptor(fixture.nodes["Contract"].clone())?;
    undescribed.with_property_value("Title", "now described")?;
    let report = validate_subject_candidates(&fixture.context, &candidates)?;
    assert!(report.is_accepted());
    assert_eq!(report.violation_count(), 0);
    for candidate in &candidates {
        assert_eq!(candidate.validation_state()?, ValidationState::Validated);
        assert!(candidate.validation_findings()?.is_empty());
    }
    assert_eq!(
        missing_title.commit_errors()?,
        vec![HolonError::NotImplemented("prior persistence failure".into())]
    );
    Ok(())
}

#[test]
fn subject_gate_assessment_error_installs_no_partial_outcomes() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("first".into())))?;
    let first = fixture.context.mutation().stage_new_holon(transient)?;
    let failing = fixture.staged_subject("failing")?;
    let stale_findings =
        validate_subject_candidates(&fixture.context, std::slice::from_ref(&first))?.violations;
    first.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    failing.replace_validation_outcome(ValidationState::Invalid, stale_findings)?;
    // The first subject can produce a NoDescriptor finding before the second encounters
    // an operational descriptor-read failure. Neither prior outcome may be replaced.
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    let candidates = [first, failing];
    let before: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            candidate
                .get_holon_to_commit(&fixture.context)
                .map(|holon| holon.read().expect("test staged holon lock").clone())
        })
        .collect::<Result<_, HolonError>>()?;

    let validation_context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(
        HolonValidationSubject { holon: &HolonReference::from(&candidates[0]) },
        &validation_context,
        &mut collector,
    )?;
    assert_eq!(collector.into_report().violation_count(), 1);
    let expected_error = validate_holon(
        HolonValidationSubject { holon: &HolonReference::from(&candidates[1]) },
        &validation_context,
        &mut ValidationCollector::default(),
    )
    .expect_err("missing property TypeName prevents reliable assessment");

    assert_eq!(validate_subject_candidates(&fixture.context, &candidates), Err(expected_error));
    for (candidate, before) in candidates.iter().zip(before) {
        assert_eq!(
            *candidate
                .get_holon_to_commit(&fixture.context)?
                .read()
                .expect("test staged holon lock"),
            before
        );
    }
    Ok(())
}

#[test]
fn terminal_candidate_is_refused_before_any_outcome_installation() -> Result<(), HolonError> {
    for committed in [false, true] {
        let fixture = Fixture::new()?;
        let first = fixture.staged_subject("first")?;
        first.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
        let terminal = fixture.staged_subject("terminal")?;
        {
            let rc_holon = terminal.get_holon_to_commit(&fixture.context)?;
            let mut holon = rc_holon.write().expect("test staged holon lock");
            let Holon::Staged(staged) = &mut *holon else {
                unreachable!("staged reference must resolve to a staged holon");
            };
            // Model both terminal lifecycle states without invoking persistence.
            if committed {
                staged.to_committed(core_types::LocalId(vec![1]))?;
            } else {
                staged.abandon_staged_changes()?;
            }
        }
        let candidates = [first, terminal];
        let before: Vec<_> = candidates
            .iter()
            .map(|candidate| {
                candidate
                    .get_holon_to_commit(&fixture.context)
                    .map(|holon| holon.read().expect("test staged holon lock").clone())
            })
            .collect::<Result<_, HolonError>>()?;

        // The first candidate has a missing required Title. Its prepared Invalid outcome
        // must be discarded when the later terminal entry reveals an invalid workset.
        assert!(matches!(
            validate_subject_candidates(&fixture.context, &candidates),
            Err(HolonError::InvalidParameter(_))
        ));
        for (candidate, before) in candidates.iter().zip(before) {
            assert_eq!(
                *candidate
                    .get_holon_to_commit(&fixture.context)?
                    .read()
                    .expect("test staged holon lock"),
                before
            );
        }
    }
    Ok(())
}

#[test]
fn subject_gate_anchor_resolution_error_preserves_prior_outcome() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("subject".into())))?;
    let candidate = fixture.context.mutation().stage_new_holon(transient)?;
    candidate.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    assert!(matches!(
        validate_subject_candidates(&fixture.context, std::slice::from_ref(&candidate)),
        Err(HolonError::HolonNotFound(_))
    ));
    assert_eq!(candidate.validation_state()?, ValidationState::Validated);
    assert!(candidate.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn empty_subject_workset_is_accepted_without_schema_anchors() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let report = validate_subject_candidates(&fixture.context, &[])?;
    assert!(report.is_accepted());
    assert_eq!(report.violation_count(), 0);
    Ok(())
}

#[test]
fn all_five_native_handlers_accept_and_reject_without_configured_evaluation(
) -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let values = [
        BaseValue::StringValue(MapString("text".into())),
        BaseValue::IntegerValue(MapInteger(42)),
        BaseValue::BooleanValue(MapBoolean(true)),
        BaseValue::BytesValue(MapBytes(vec![1, 2])),
        BaseValue::EnumValue(MapEnumValue(MapString("member".into()))),
    ];
    let path = value_path();
    for (index, (rule, _, target)) in RULES[2..].iter().enumerate() {
        let descriptor = ValueDescriptor::from_holon(fixture.nodes[*target].clone());
        for (value_index, value) in values.iter().enumerate() {
            let mut collector = ValidationCollector::default();
            validate_value(
                ValueValidationSubject { descriptor: &descriptor, value, path: &path },
                &context,
                &mut collector,
            )?;
            assert!(collector.observations().dispatched_rule_keys.contains(rule.as_str()));
            let report = collector.into_report();
            assert_eq!(report.is_accepted(), index == value_index);
            assert_eq!(report.violation_count(), usize::from(index != value_index));
        }
    }
    Ok(())
}

#[test]
fn full_contract_traversal_assesses_absent_required_properties() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut subject = fixture.subject()?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
    );
    assert!(
        matches!(&report.violations[0].subject, ValidationSubjectPath::Property { name, .. } if name == "Title")
    );
    subject.with_property_value("Title", "present")?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert_eq!(collector.observations().effective_constraint_count, 0);
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn property_minimum_exemption_does_not_skip_populated_value_validation() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let values = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor = PropertyDescriptor::from_holon(fixture.nodes["Title.PropertyType"].clone());
    let path =
        ValidationSubjectPath::Property { holon_identity: "abstract".into(), name: "Title".into() };
    let context = PropertyValidationContext { enforce_minimum: false, values: &values };
    let mismatch = BaseValue::IntegerValue(MapInteger(3));
    for value in [None, Some(&mismatch)] {
        let mut collector = ValidationCollector::default();
        validate_property(
            PropertyValidationSubject { descriptor: &descriptor, value, path: &path },
            &context,
            &mut collector,
        )?;
        assert_eq!(collector.into_report().is_accepted(), value.is_none());
    }
    Ok(())
}

#[test]
fn undescribed_property_policy_always_rejects() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let mut subject = fixture.subject()?;
    subject.with_property_value("Title", "present")?.with_property_value("Extra", true)?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-003")
    );
    Ok(())
}

#[test]
fn missing_and_multiple_descriptors_stop_bootstrap_with_distinct_findings() -> Result<(), HolonError>
{
    let fixture = Fixture::new()?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut subject: holons_core::HolonReference =
        fixture.context.mutation().new_holon(Some(MapString("undescribed".into())))?.into();
    let mut messages = Vec::new();
    for multiple in [false, true] {
        if multiple {
            subject.add_related_holons(
                CoreRelationshipTypeName::DescribedBy,
                vec![fixture.nodes["Contract"].clone(), fixture.nodes["TypeDescriptor"].clone()],
            )?;
        }
        let mut collector = ValidationCollector::default();
        validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
        assert!(collector.observations().dispatched_rule_keys.is_empty());
        let report = collector.into_report();
        assert_eq!(report.violation_count(), 1);
        assert_eq!(report.violations[0].kind, CommitValidationViolationKind::NoDescriptor);
        messages.push(report.violations[0].message.clone());
    }
    assert!(messages[0].starts_with("MissingDescribedBy:"));
    assert!(messages[1].starts_with("MultipleDescribedBy:"));
    Ok(())
}

#[test]
fn incompatible_binding_is_rejected_before_its_handler_dispatch() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let key = CoreValidationRuleName::BaseValueKindMatchesString.as_str();
    fixture.link("Title.PropertyType", CoreRelationshipTypeName::ValidationBindings, key)?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor = PropertyDescriptor::from_holon(fixture.nodes["Title.PropertyType"].clone());
    let path = property_path("Title");
    let mut collector = ValidationCollector::default();
    validate_property(
        PropertyValidationSubject { descriptor: &descriptor, value: None, path: &path },
        &PropertyValidationContext { enforce_minimum: false, values: &context },
        &mut collector,
    )?;
    assert!(!collector.observations().dispatched_rule_keys.contains(key));
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "IncompatibleValidationBinding")
    );
    assert_eq!(
        report.violations[0].descriptor_identity,
        Some(descriptor.holon().reference_id_string())
    );
    Ok(())
}

#[test]
fn unsupported_rules_and_constraints_fail_closed_with_contribution_provenance(
) -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    fixture.node("Unknown.ValidationRule")?;
    fixture.node("Constraint.Type")?;
    fixture.node("Constraint.Instance")?;
    fixture.link(
        "Unknown.ValidationRule",
        CoreRelationshipTypeName::DescribedBy,
        "StringValidationRule.HolonType",
    )?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::ValidationBindings,
        "Unknown.ValidationRule",
    )?;
    fixture.link(
        "Constraint.Instance",
        CoreRelationshipTypeName::DescribedBy,
        "Constraint.Type",
    )?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "Constraint.Instance",
    )?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor =
        ValueDescriptor::from_holon(fixture.nodes["StringValueType.ValueType"].clone());
    let value = BaseValue::IntegerValue(MapInteger(5));
    let mut collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
        &context,
        &mut collector,
    )?;
    assert_eq!(collector.observations().effective_constraint_count, 1);
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 3);
    assert_eq!(report.violations[0].kind, CommitValidationViolationKind::UnsupportedValidationRule);
    assert_eq!(
        report.violations[1].kind,
        CommitValidationViolationKind::UnsupportedConstraintType {
            constraint_identity: fixture.nodes["Constraint.Instance"].reference_id_string(),
            constraint_type_identity: fixture.nodes["Constraint.Type"].reference_id_string(),
        }
    );
    assert_eq!(
        report.violations[1].descriptor_identity,
        Some(descriptor.holon().reference_id_string())
    );
    Ok(())
}

#[test]
fn subject_rule_registry_covers_fixture_bindings_and_report_rejects_findings() {
    for (rule, _, _) in RULES {
        assert!(StaticRuleRegistry::lookup(&ValidationRuleKey(rule.as_str().into())).is_some());
    }
    assert!(
        StaticRuleRegistry::lookup(&ValidationRuleKey("Unknown.ValidationRule".into())).is_none()
    );
    assert!(
        StaticConstraintRegistry::lookup(&ConstraintTypeKey("Configured.Type".into())).is_none()
    );
    assert!(CommitValidationReport::default().is_accepted());
    let mut collector = ValidationCollector::default();
    for severity in [
        core_types::ValidationSeverity::Info,
        core_types::ValidationSeverity::Warning,
        core_types::ValidationSeverity::Error,
    ] {
        collector.record(core_types::CommitValidationViolation {
            kind: CommitValidationViolationKind::UnsupportedValidationRule,
            rule_key: None,
            severity,
            subject: value_path(),
            descriptor_identity: None,
            message: format!("{severity:?}"),
        });
    }
    let report = collector.into_report();
    assert!(!report.is_accepted());
    assert_eq!(report.violation_count(), 3);
    assert_eq!(
        report.violations.iter().map(|finding| finding.message.as_str()).collect::<Vec<_>>(),
        ["Info", "Warning", "Error"]
    );
}

#[test]
fn inactive_rules_do_not_dispatch_and_optional_absence_is_accepted() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let unbound = fixture.node("Unbound.ValueType")?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor = ValueDescriptor::from_holon(unbound);
    let value = BaseValue::IntegerValue(MapInteger(3));
    let mut collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
        &context,
        &mut collector,
    )?;
    assert!(collector.observations().discovered_rule_keys.is_empty());
    assert!(collector.observations().dispatched_rule_keys.is_empty());
    assert!(collector.into_report().is_accepted());

    let descriptor = PropertyDescriptor::from_holon(fixture.nodes["Key.PropertyType"].clone());
    let mut collector = ValidationCollector::default();
    validate_property(
        PropertyValidationSubject {
            descriptor: &descriptor,
            value: None,
            path: &property_path("Key"),
        },
        &PropertyValidationContext { enforce_minimum: true, values: &context },
        &mut collector,
    )?;
    assert!(collector
        .observations()
        .dispatched_rule_keys
        .contains(CoreValidationRuleName::RequiredPropertyPresence.as_str()));
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn abstract_descriptor_exemption_is_computed_from_the_universal_contract() -> Result<(), HolonError>
{
    let mut fixture = Fixture::new()?;
    fixture.node("IsAbstractType.PropertyType")?;
    fixture
        .nodes
        .get_mut("IsAbstractType.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "IsAbstractType")?;
    fixture.link(
        "IsAbstractType.PropertyType",
        CoreRelationshipTypeName::Extends,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "IsAbstractType.PropertyType",
        CoreRelationshipTypeName::ValueType,
        "BooleanValueType.ValueType",
    )?;
    fixture.link(
        "Contract",
        CoreRelationshipTypeName::InstanceProperties,
        "IsAbstractType.PropertyType",
    )?;
    let mut subject = fixture.subject()?;
    subject.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![fixture.nodes["TypeDescriptor"].clone()],
    )?;
    subject.with_property_value("IsAbstractType", true)?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert!(collector.into_report().is_accepted());
    drop(context);

    // The same absent property becomes mandatory when it is part of the
    // universal descriptor contract. The exemption is not a blanket skip.
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::InstanceProperties,
        "Title.PropertyType",
    )?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
    );
    Ok(())
}

#[test]
fn binding_compatibility_uses_rule_type_lineage_and_original_declaration() -> Result<(), HolonError>
{
    let mut fixture = Fixture::new()?;
    fixture.node("StringRuleSubtype")?;
    fixture.link(
        "StringRuleSubtype",
        CoreRelationshipTypeName::Extends,
        "StringValidationRule.HolonType",
    )?;
    let rule_key = CoreValidationRuleName::BaseValueKindMatchesString.as_str();
    let original_family = fixture.nodes["StringValidationRule.HolonType"].clone();
    fixture
        .nodes
        .get_mut(rule_key)
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original_family])?;
    fixture.link(rule_key, CoreRelationshipTypeName::DescribedBy, "StringRuleSubtype")?;
    fixture.node("Derived.StringValueType")?;
    fixture.link(
        "Derived.StringValueType",
        CoreRelationshipTypeName::Extends,
        "StringValueType.ValueType",
    )?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor = ValueDescriptor::from_holon(fixture.nodes["Derived.StringValueType"].clone());
    let value = BaseValue::BooleanValue(MapBoolean(true));
    let mut collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
        &context,
        &mut collector,
    )?;
    assert!(collector.observations().dispatched_rule_keys.contains(rule_key));
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert_eq!(
        report.violations[0].descriptor_identity,
        Some(fixture.nodes["StringValueType.ValueType"].reference_id_string())
    );
    Ok(())
}

#[test]
fn canonical_rule_with_wrong_authored_family_never_dispatches() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let rule_key = CoreValidationRuleName::BaseValueKindMatchesString.as_str();
    let original_family = fixture.nodes["StringValidationRule.HolonType"].clone();
    fixture
        .nodes
        .get_mut(rule_key)
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original_family])?;
    fixture.link(
        rule_key,
        CoreRelationshipTypeName::DescribedBy,
        "IntegerValidationRule.HolonType",
    )?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor =
        ValueDescriptor::from_holon(fixture.nodes["StringValueType.ValueType"].clone());
    let value = BaseValue::StringValue(MapString("text".into()));
    let mut collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
        &context,
        &mut collector,
    )?;
    assert!(collector.observations().dispatched_rule_keys.is_empty());
    assert_eq!(collector.into_report().violation_count(), 1);
    Ok(())
}

#[test]
fn unsupported_constraints_on_holon_and_absent_property_are_not_skipped() -> Result<(), HolonError>
{
    let mut fixture = Fixture::new()?;
    fixture.node("Constraint.Type")?;
    fixture.node("Constraint.Instance")?;
    fixture.link(
        "Constraint.Instance",
        CoreRelationshipTypeName::DescribedBy,
        "Constraint.Type",
    )?;
    fixture.link(
        "HolonType.TypeDescriptor",
        CoreRelationshipTypeName::Constraints,
        "Constraint.Instance",
    )?;
    fixture.link(
        "PropertyType.TypeDescriptor",
        CoreRelationshipTypeName::Constraints,
        "Constraint.Instance",
    )?;
    let subject = fixture.subject()?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert_eq!(collector.observations().effective_constraint_count, 3);
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 4);
    assert_eq!(
        report
            .violations
            .iter()
            .filter(|finding| matches!(
                finding.kind,
                CommitValidationViolationKind::UnsupportedConstraintType { .. }
            ))
            .count(),
        3
    );
    Ok(())
}

#[test]
fn malformed_property_paths_fail_before_discovery_for_absent_and_populated_values(
) -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let values = ValueValidationContext::resolve(&fixture.context)?;
    let context = PropertyValidationContext { enforce_minimum: true, values: &values };
    let descriptor = PropertyDescriptor::from_holon(fixture.nodes["Title.PropertyType"].clone());
    let value = BaseValue::StringValue(MapString("present".into()));
    for path in [
        ValidationSubjectPath::Holon { holon_identity: "subject".into() },
        value_path(),
        ValidationSubjectPath::Transaction,
        ValidationSubjectPath::Relationship {
            source_identity: "subject".into(),
            name: "Related".into(),
            target_identity: "target".into(),
        },
    ] {
        for value in [None, Some(&value)] {
            let mut collector = ValidationCollector::default();
            assert!(matches!(
                validate_property(
                    PropertyValidationSubject { descriptor: &descriptor, value, path: &path },
                    &context,
                    &mut collector,
                ),
                Err(HolonError::InvalidParameter(_))
            ));
            assert_eq!(collector.observations(), &ValidationObservations::default());
        }
    }
    Ok(())
}

#[test]
fn unresolved_constraint_type_is_an_incomplete_assessment() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    fixture.node("Undescribed.Constraint")?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "Undescribed.Constraint",
    )?;
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor =
        ValueDescriptor::from_holon(fixture.nodes["StringValueType.ValueType"].clone());
    let value = BaseValue::StringValue(MapString("present".into()));
    let mut collector = ValidationCollector::default();
    assert!(matches!(
        validate_value(
            ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
            &context,
            &mut collector,
        ),
        Err(HolonError::MissingDescribedBy { .. })
    ));
    assert_eq!(collector.observations().effective_constraint_count, 1);
    assert_eq!(collector.observations().constraint_declaration_count, 0);
    Ok(())
}

#[test]
fn incomplete_schema_prevents_constructing_a_validation_context() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    HolonValidationContext::resolve(&fixture.context)?;
    let mut incomplete = Fixture::empty()?;
    for key in fixture.nodes.keys() {
        if key != CoreValidationRuleName::SchemaDependenciesAcyclic.as_str() {
            incomplete.node(key)?;
        }
    }
    assert!(matches!(
        HolonValidationContext::resolve(&incomplete.context),
        Err(HolonError::HolonNotFound(_))
    ));
    Ok(())
}

#[test]
fn subject_gate_rejects_unlicensed_relationships_assembled_before_descriptors(
) -> Result<(), HolonError> {
    for name in ["AuthorOf", "UnknownRelationship"] {
        let fixture = Fixture::new()?;
        let mut clean = fixture.staged_subject("clean-peer")?;
        clean.with_property_value("Title", "clean")?;
        let mut input = fixture.context.mutation().new_holon(Some("raw-input".into()))?;
        input.add_related_holons(name, vec![clean.clone().into()])?;
        let mut candidate = fixture.context.mutation().stage_new_holon(input)?;
        candidate.with_descriptor(fixture.nodes["Contract"].clone())?;
        candidate.with_property_value("Title", "invalid relationship")?;
        let candidates = [clean.clone(), candidate.clone()];
        let report = validate_subject_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(report.violation_count(), 1);
        assert!(matches!(&report.violations[0].kind,
            CommitValidationViolationKind::RuleViolation { code } if code == "UndeclaredRelationship"));
        assert!(matches!(&report.violations[0].subject,
            ValidationSubjectPath::Relationship { name: actual, .. } if actual == name));
        assert_eq!(candidate.validation_state()?, ValidationState::Invalid);
        assert_eq!(clean.validation_state()?, ValidationState::Validated);
        assert!(candidate.commit_errors()?.is_empty());
        // Empty collections left by removal are not authored occurrences.
        candidate.remove_related_holons(name, vec![clean.into()])?;
        assert!(validate_subject_candidates(&fixture.context, &candidates)?.is_accepted());
        assert!(candidate.validation_findings()?.is_empty());
    }
    Ok(())
}

#[test]
fn relationship_authoring_check_is_specific_to_staged_candidates() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let mut subject = fixture.subject()?;
    subject.with_property_value("Title", "read surface")?;
    subject.add_related_holons("AuthorOf", vec![fixture.nodes["Contract"].clone()])?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn subject_gate_accepts_inherited_declarations_without_target_descriptors() -> Result<(), HolonError>
{
    let mut fixture = Fixture::new()?;
    fixture.node("Forward.Relationship")?;
    fixture
        .nodes
        .get_mut("Forward.Relationship")
        .unwrap()
        .with_property_value("TypeName", "Forward")?;
    fixture.link(
        "Forward.Relationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType",
    )?;
    fixture.link(
        "HolonType.TypeDescriptor",
        CoreRelationshipTypeName::InstanceRelationships,
        "Forward.Relationship",
    )?;
    let mut candidate = fixture.staged_subject("candidate")?;
    candidate.with_property_value("Title", "declared source")?;
    // No target descriptor exists. Declared-write authorization is source-only.
    let target = fixture.node("undescribed-target")?;
    candidate.add_related_holons_ungoverned("Forward", vec![target])?;
    assert!(validate_subject_candidates(&fixture.context, &[candidate])?.is_accepted());
    Ok(())
}

#[test]
fn malformed_relationship_contract_aborts_before_installing_outcomes() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let missing = fixture.context.mutation().new_holon(Some("undescribed".into()))?;
    let first = fixture.context.mutation().stage_new_holon(missing)?;
    first.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    let candidate = fixture.staged_subject("candidate")?;
    fixture.nodes.get_mut("DescribedBy.Relationship").unwrap().remove_property_value("TypeName")?;
    assert!(
        validate_subject_candidates(&fixture.context, &[first.clone(), candidate.clone()]).is_err()
    );
    assert_eq!(first.validation_state()?, ValidationState::Validated);
    assert!(first.validation_findings()?.is_empty());
    assert_eq!(candidate.validation_state()?, ValidationState::ValidationRequired);
    assert!(candidate.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn aggregate_only_report_rejects_without_installing_staged_outcomes() -> Result<(), HolonError> {
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    let aggregate = core_types::CommitValidationViolation {
        kind: CommitValidationViolationKind::RuleViolation { code: "AggregateTest".into() },
        rule_key: None,
        severity: core_types::ValidationSeverity::Error,
        subject: ValidationSubjectPath::Holon { holon_identity: "unstaged-schema".into() },
        descriptor_identity: None,
        message: "Aggregate finding without a staged carrier.".into(),
    };
    assessment.push_aggregate(aggregate.clone());

    let report = assessment.install_outcomes()?;
    assert!(!report.is_accepted());
    assert_eq!(report.violation_count(), 1);
    assert_eq!(report.violations, vec![aggregate]);
    assert_eq!(report.unattached_findings()?, vec![&report.violations[0]]);
    Ok(())
}

#[test]
fn aggregate_findings_do_not_leak_into_candidate_outcomes() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let invalid = fixture.staged_subject("invalid")?;
    let clean = fixture.staged_subject("clean")?;
    let candidate_report =
        validate_subject_candidates(&fixture.context, std::slice::from_ref(&invalid))?;
    let mut aggregate = candidate_report.violations[0].clone();
    aggregate.subject = ValidationSubjectPath::Holon { holon_identity: "unstaged-schema".into() };
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    assessment.push_aggregate(aggregate.clone());
    assessment.record_candidate(&invalid, candidate_report.clone());
    assessment.push_aggregate(aggregate.clone());
    assessment.record_candidate(&clean, CommitValidationReport::default());
    assessment.push_aggregate(aggregate.clone());

    let report = assessment.install_outcomes()?;
    assert!(!report.is_accepted());
    assert_eq!(report.violation_count(), 4);
    assert_eq!(report.unattached_findings()?, vec![&aggregate; 3]);
    assert_eq!(invalid.validation_findings()?, candidate_report.violations);
    assert_eq!(clean.validation_state()?, ValidationState::Validated);
    assert!(clean.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn identical_candidate_and_aggregate_findings_keep_distinct_carriers() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let candidate = fixture.staged_subject("candidate")?;
    let candidate_report =
        validate_subject_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
    let finding = candidate_report.violations[0].clone();
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    assessment.record_candidate(&candidate, candidate_report);
    assessment.push_aggregate(finding.clone());

    let report = assessment.install_outcomes()?;
    assert_eq!(report.violations, vec![finding.clone(), finding.clone()]);
    assert_eq!(candidate.validation_findings()?, vec![finding.clone()]);
    assert_eq!(report.unattached_findings()?, vec![&finding]);
    Ok(())
}

fn c2_kind_roots(fixture: &mut Fixture) -> Result<holons_core::DescriptorKindRoots, HolonError> {
    use type_names::CorePropertyTypeName;
    fixture.node("MetaHolonType.MetaTypeDescriptor")?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "MetaHolonType.MetaTypeDescriptor",
        CoreRelationshipTypeName::Extends,
        "MetaTypeDescriptor.HolonType",
    )?;
    for (key, anchor) in [
        ("TypeDescriptor", false),
        ("HolonType.TypeDescriptor", true),
        ("MetaTypeDescriptor.HolonType", false),
        ("MetaHolonType.MetaTypeDescriptor", false),
        ("PropertyType.TypeDescriptor", true),
        ("Contract", false),
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value(CorePropertyTypeName::DefinesInstanceTypeKind, anchor)?;
    }
    fixture
        .nodes
        .get_mut("HolonType.TypeDescriptor")
        .unwrap()
        .with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
    holons_core::DescriptorKindRoots::from_resolved(
        &fixture.context,
        fixture.nodes["TypeDescriptor"].clone(),
        fixture.nodes["HolonType.TypeDescriptor"].clone(),
        fixture.nodes["MetaTypeDescriptor.HolonType"].clone(),
        fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone(),
    )
}

fn dispatch_c2_rule(
    fixture: &mut Fixture,
    rule: CoreValidationRuleName,
    products: &DescriptorRuleProducts,
    subject: &HolonReference,
) -> Result<Vec<core_types::CommitValidationViolation>, HolonError> {
    fixture.node(rule.as_str())?;
    let binding = ResolvedValidationBinding {
        rule: fixture.nodes[rule.as_str()].clone(),
        declaring_descriptor: holons_core::HolonDescriptor::from_holon(
            fixture.nodes["MetaTypeDescriptor.HolonType"].clone(),
        ),
    };
    let path = ValidationSubjectPath::Holon { holon_identity: subject.reference_id_string() };
    let mut collector = ValidationCollector::default();
    StaticRuleRegistry::lookup(&ValidationRuleKey(rule.as_str().into())).unwrap()(
        ValidationInvocation::Descriptor { binding: &binding, path: &path, products },
        &mut collector,
    )?;
    Ok(collector.into_report().violations)
}

#[test]
fn all_thirteen_c2_handlers_are_registered_but_have_no_native_value_kind() {
    use CoreValidationRuleName::*;
    for rule in [
        AtMostOneDirectParent,
        AcyclicExtendsLineage,
        ExtendsLineageTerminatesAtTypeDescriptor,
        UniqueTypeDescriptorRoot,
        LocalInstanceKindAnchorDesignation,
        InstanceKindAnchorsAreAbstract,
        TypeDescriptorRootKindException,
        DescribingCategoryCompatibility,
        DescriptorMetaTypeCorrespondence,
        NoInheritedMemberRedeclaration,
        UniqueSemanticMemberNames,
        WellFormedEffectiveMemberDefinitions,
        ContractMemberKindCompatibility,
    ] {
        assert!(StaticRuleRegistry::lookup(&ValidationRuleKey(rule.as_str().into())).is_some());
        assert_eq!(crate::handlers::native_rule_kind(rule), None);
    }
}

#[test]
fn structural_handler_reports_multiple_parents_from_drained_kernel_diagnosis(
) -> Result<(), HolonError> {
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    fixture.node("OtherParent")?;
    fixture.link("Contract", CoreRelationshipTypeName::Extends, "OtherParent")?;
    let subject = fixture.nodes["Contract"].clone();
    let prerequisites =
        StructuralPrerequisites::assess(&subject, &fixture.nodes["TypeDescriptor"])?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_structure(&prerequisites, &fixture.nodes["TypeDescriptor"]);
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::AtMostOneDirectParent,
        &products,
        &subject,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(
        matches!(&findings[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-STRUCT-002")
    );
    assert!(findings[0].message.contains("2 direct Extends parents"));
    Ok(())
}

#[test]
fn structural_handlers_translate_cycle_termination_and_root_defects() -> Result<(), HolonError> {
    use holons_core::StructuralPrerequisites;
    for (rule, code, subject_key) in [
        (CoreValidationRuleName::AcyclicExtendsLineage, "DS-STRUCT-003", "A"),
        (
            CoreValidationRuleName::ExtendsLineageTerminatesAtTypeDescriptor,
            "DS-STRUCT-004",
            "Child",
        ),
        (CoreValidationRuleName::UniqueTypeDescriptorRoot, "DS-STRUCT-005", "TypeDescriptor"),
    ] {
        let mut fixture = Fixture::new()?;
        match subject_key {
            "A" => {
                fixture.node("A")?;
                fixture.node("B")?;
                fixture.link("A", CoreRelationshipTypeName::Extends, "B")?;
                fixture.link("B", CoreRelationshipTypeName::Extends, "A")?;
            }
            "Child" => {
                fixture.node("Child")?;
                fixture.node("Unrooted")?;
                fixture.link("Child", CoreRelationshipTypeName::Extends, "Unrooted")?;
            }
            "TypeDescriptor" => {
                fixture.node("Unrooted")?;
                fixture.link("TypeDescriptor", CoreRelationshipTypeName::Extends, "Unrooted")?;
            }
            _ => unreachable!(),
        }
        let subject = fixture.nodes[subject_key].clone();
        let prerequisites =
            StructuralPrerequisites::assess(&subject, &fixture.nodes["TypeDescriptor"])?;
        let mut products = DescriptorRuleProducts::default();
        products.prepare_structure(&prerequisites, &fixture.nodes["TypeDescriptor"]);
        let findings = dispatch_c2_rule(&mut fixture, rule, &products, &subject)?;
        assert!(findings.iter().any(|finding| matches!(&finding.kind,
            CommitValidationViolationKind::RuleViolation { code: actual } if actual == code)));
    }
    Ok(())
}

#[test]
fn rootless_holon_is_not_inferred_to_be_a_descriptor_from_its_meta_type() -> Result<(), HolonError>
{
    use holons_core::{CurrentDescriptorReader, StructuralPrerequisites};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    fixture.node("Rootless")?;
    fixture.link(
        "Rootless",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let subject = fixture.nodes["Rootless"].clone();
    let prerequisites = StructuralPrerequisites::assess(&subject, &roots.type_descriptor)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_structure(&prerequisites, &roots.type_descriptor);
    products.prepare_kind(&prerequisites, &roots, &CurrentDescriptorReader)?;
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::UniqueTypeDescriptorRoot,
        &products,
        &subject
    )?
    .is_empty());
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescriptorMetaTypeCorrespondence,
        &products,
        &subject,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(matches!(&findings[0].kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-005"));
    Ok(())
}

#[test]
fn kind_handlers_diagnose_local_anchor_designation_and_abstractness() -> Result<(), HolonError> {
    use holons_core::{CurrentDescriptorReader, StructuralPrerequisites};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    let subject = fixture.nodes["Contract"].clone();
    let prerequisites = StructuralPrerequisites::assess(&subject, &roots.type_descriptor)?;
    fixture
        .nodes
        .get_mut("Contract")
        .unwrap()
        .remove_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind)?;
    let mut missing = DescriptorRuleProducts::default();
    missing.prepare_kind(&prerequisites, &roots, &CurrentDescriptorReader)?;
    assert_eq!(
        dispatch_c2_rule(
            &mut fixture,
            CoreValidationRuleName::LocalInstanceKindAnchorDesignation,
            &missing,
            &subject,
        )?
        .len(),
        1
    );
    fixture
        .nodes
        .get_mut("Contract")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?;
    let mut non_abstract = DescriptorRuleProducts::default();
    non_abstract.prepare_kind(&prerequisites, &roots, &CurrentDescriptorReader)?;
    assert_eq!(
        dispatch_c2_rule(
            &mut fixture,
            CoreValidationRuleName::InstanceKindAnchorsAreAbstract,
            &non_abstract,
            &subject,
        )?
        .len(),
        1
    );
    Ok(())
}

#[test]
fn kind_handlers_reject_an_ordinary_holon_with_an_incompatible_describer() -> Result<(), HolonError>
{
    use holons_core::{CurrentDescriptorReader, StructuralPrerequisites};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    let mut subject = fixture.context.mutation().new_holon(Some("ordinary".into()))?;
    subject.with_descriptor(fixture.nodes["StringValueType.ValueType"].clone())?;
    let subject: HolonReference = subject.into();
    let prerequisites = StructuralPrerequisites::assess(&subject, &roots.type_descriptor)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &roots, &CurrentDescriptorReader)?;
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescribingCategoryCompatibility,
        &products,
        &subject,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(
        matches!(&findings[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-004")
    );
    Ok(())
}

#[test]
fn kind_handlers_cover_the_root_exception_and_self_description_without_recursion(
) -> Result<(), HolonError> {
    use holons_core::{CurrentDescriptorReader, StructuralPrerequisites};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    let meta_holon = fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone();
    fixture
        .nodes
        .get_mut("MetaHolonType.MetaTypeDescriptor")
        .unwrap()
        .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![meta_holon.clone()])?;
    let prerequisites = StructuralPrerequisites::assess(&meta_holon, &roots.type_descriptor)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &roots, &CurrentDescriptorReader)?;
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescriptorMetaTypeCorrespondence,
        &products,
        &meta_holon,
    )?
    .is_empty());

    let root = fixture.nodes["TypeDescriptor"].clone();
    let root_prerequisites = StructuralPrerequisites::assess(&root, &root)?;
    let mut root_products = DescriptorRuleProducts::default();
    root_products.prepare_kind(&root_prerequisites, &roots, &CurrentDescriptorReader)?;
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::TypeDescriptorRootKindException,
        &root_products,
        &root,
    )?
    .is_empty());
    fixture
        .nodes
        .get_mut("TypeDescriptor")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?;
    let mut invalid_root_products = DescriptorRuleProducts::default();
    invalid_root_products.prepare_kind(&root_prerequisites, &roots, &CurrentDescriptorReader)?;
    assert_eq!(
        dispatch_c2_rule(
            &mut fixture,
            CoreValidationRuleName::TypeDescriptorRootKindException,
            &invalid_root_products,
            &root,
        )?
        .len(),
        1
    );
    Ok(())
}

#[test]
fn contract_handlers_keep_redeclaration_name_and_definition_defects_distinct(
) -> Result<(), HolonError> {
    use holons_core::{ContractContributions, CurrentDescriptorReader};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    fixture.node("Subtype")?;
    fixture.link("Subtype", CoreRelationshipTypeName::Extends, "Contract")?;
    fixture.link("Subtype", CoreRelationshipTypeName::InstanceProperties, "Title.PropertyType")?;
    fixture.node("Alias.PropertyType")?;
    fixture
        .nodes
        .get_mut("Alias.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "Title")?;
    fixture.link(
        "Alias.PropertyType",
        CoreRelationshipTypeName::Extends,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link("Subtype", CoreRelationshipTypeName::InstanceProperties, "Alias.PropertyType")?;
    fixture.node("WrongKind")?;
    fixture
        .nodes
        .get_mut("WrongKind")
        .unwrap()
        .with_property_value("TypeName", "Wrong")?
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?;
    fixture.link("WrongKind", CoreRelationshipTypeName::Extends, "StringValueType.ValueType")?;
    fixture.link("WrongKind", CoreRelationshipTypeName::ValueType, "StringValueType.ValueType")?;
    fixture.link("Subtype", CoreRelationshipTypeName::InstanceProperties, "WrongKind")?;
    fixture.node("BadValue.PropertyType")?;
    fixture
        .nodes
        .get_mut("BadValue.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "BadValue")?
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, false)?;
    fixture.link(
        "BadValue.PropertyType",
        CoreRelationshipTypeName::Extends,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "BadValue.PropertyType",
        CoreRelationshipTypeName::ValueType,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "Subtype",
        CoreRelationshipTypeName::InstanceProperties,
        "BadValue.PropertyType",
    )?;
    let subject = fixture.nodes["Subtype"].clone();
    let contributions = ContractContributions::resolve(&subject)?;
    let contract_roots = crate::descriptor_rules::ContractKindRoots {
        property: fixture.nodes["PropertyType.TypeDescriptor"].clone(),
        relationship: fixture.nodes["DeclaredRelationshipType"].clone(),
        value: fixture.nodes["StringValueType.ValueType"].clone(),
    };
    let mut products = DescriptorRuleProducts::default();
    products.prepare_contract(&contributions, &roots, &contract_roots, &CurrentDescriptorReader)?;
    for (rule, code) in [
        (CoreValidationRuleName::NoInheritedMemberRedeclaration, "DS-CONTRACT-001"),
        (CoreValidationRuleName::UniqueSemanticMemberNames, "DS-CONTRACT-002"),
        (CoreValidationRuleName::WellFormedEffectiveMemberDefinitions, "DS-CONTRACT-003"),
        (CoreValidationRuleName::ContractMemberKindCompatibility, "DS-CONTRACT-004"),
    ] {
        let findings = dispatch_c2_rule(&mut fixture, rule, &products, &subject)?;
        assert!(findings.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code: actual } if actual == code)));
        if rule == CoreValidationRuleName::UniqueSemanticMemberNames {
            assert!(findings.iter().any(|finding| finding
                .message
                .contains(&fixture.nodes["Contract"].reference_id_string())
                && finding.message.contains(&subject.reference_id_string())));
        }
        if rule == CoreValidationRuleName::WellFormedEffectiveMemberDefinitions {
            for edge in ["SourceType", "TargetType"] {
                assert_eq!(
                    findings
                        .iter()
                        .filter(|finding| finding
                            .message
                            .contains(&format!("exactly one {edge} target")))
                        .count(),
                    1
                );
            }
        }
        if rule == CoreValidationRuleName::ContractMemberKindCompatibility {
            assert!(findings
                .iter()
                .any(|finding| finding.message.contains("property ValueType target")));
        }
    }
    Ok(())
}

#[test]
fn additive_subtype_members_in_both_namespaces_need_no_open_content_flags() -> Result<(), HolonError>
{
    use holons_core::{ContractContributions, CurrentDescriptorReader};
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_roots(&mut fixture)?;
    fixture
        .nodes
        .get_mut("StringValueType.ValueType")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?
        .with_property_value(type_names::CorePropertyTypeName::IsAbstractType, true)?;
    for key in ["Title.PropertyType", "Key.PropertyType", "DescribedBy.Relationship"] {
        fixture.nodes.get_mut(key).unwrap().with_property_value(
            type_names::CorePropertyTypeName::DefinesInstanceTypeKind,
            false,
        )?;
    }
    fixture.node("DeclaredRelationshipType.RelationshipType")?;
    fixture.link(
        "DeclaredRelationshipType.RelationshipType",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    fixture
        .nodes
        .get_mut("DeclaredRelationshipType.RelationshipType")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?
        .with_property_value(type_names::CorePropertyTypeName::IsAbstractType, true)?;
    fixture.link(
        "DeclaredRelationshipType",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture
        .nodes
        .get_mut("DeclaredRelationshipType")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, false)?;
    for edge in [CoreRelationshipTypeName::SourceType, CoreRelationshipTypeName::TargetType] {
        fixture.link("DescribedBy.Relationship", edge, "HolonType.TypeDescriptor")?;
    }
    fixture.node("Subtype")?;
    fixture.link("Subtype", CoreRelationshipTypeName::Extends, "Contract")?;
    fixture.node("Subtitle.PropertyType")?;
    fixture
        .nodes
        .get_mut("Subtitle.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "Subtitle")?;
    fixture
        .nodes
        .get_mut("Subtitle.PropertyType")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, false)?;
    fixture.link(
        "Subtitle.PropertyType",
        CoreRelationshipTypeName::Extends,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "Subtitle.PropertyType",
        CoreRelationshipTypeName::ValueType,
        "StringValueType.ValueType",
    )?;
    fixture.link(
        "Subtype",
        CoreRelationshipTypeName::InstanceProperties,
        "Subtitle.PropertyType",
    )?;
    fixture.node("SubtypeLink.Relationship")?;
    fixture
        .nodes
        .get_mut("SubtypeLink.Relationship")
        .unwrap()
        .with_property_value("TypeName", "SubtypeLink")?;
    fixture
        .nodes
        .get_mut("SubtypeLink.Relationship")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, false)?;
    fixture.link(
        "SubtypeLink.Relationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link("SubtypeLink.Relationship", CoreRelationshipTypeName::SourceType, "Subtype")?;
    fixture.link("SubtypeLink.Relationship", CoreRelationshipTypeName::TargetType, "Contract")?;
    fixture.link(
        "Subtype",
        CoreRelationshipTypeName::InstanceRelationships,
        "SubtypeLink.Relationship",
    )?;

    let subtype = fixture.nodes["Subtype"].clone();
    let contributions = ContractContributions::resolve(&subtype)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_contract(
        &contributions,
        &roots,
        &crate::descriptor_rules::ContractKindRoots {
            property: fixture.nodes["PropertyType.TypeDescriptor"].clone(),
            relationship: fixture.nodes["DeclaredRelationshipType.RelationshipType"].clone(),
            value: fixture.nodes["StringValueType.ValueType"].clone(),
        },
        &CurrentDescriptorReader,
    )?;
    assert!(!products.has_findings());
    for rule in [
        CoreValidationRuleName::NoInheritedMemberRedeclaration,
        CoreValidationRuleName::UniqueSemanticMemberNames,
        CoreValidationRuleName::WellFormedEffectiveMemberDefinitions,
        CoreValidationRuleName::ContractMemberKindCompatibility,
    ] {
        assert!(dispatch_c2_rule(&mut fixture, rule, &products, &subtype)?.is_empty());
    }
    Ok(())
}

#[test]
fn invalidated_unattached_position_returns_error_without_panicking() -> Result<(), HolonError> {
    let finding = core_types::CommitValidationViolation {
        kind: CommitValidationViolationKind::RuleViolation { code: "AggregateTest".into() },
        rule_key: None,
        severity: core_types::ValidationSeverity::Error,
        subject: ValidationSubjectPath::Holon { holon_identity: "schema".into() },
        descriptor_identity: None,
        message: "Schema finding".into(),
    };
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    assessment.push_aggregate(finding);
    let mut report = assessment.install_outcomes()?;
    report.violations.clear();
    assert!(matches!(report.unattached_findings(), Err(HolonError::CommitFailure(_))));
    Ok(())
}

#[test]
fn extension_binding_checks_all_matching_roots_regardless_of_order() -> Result<(), HolonError> {
    use crate::contexts::SubjectLevel;
    for reverse in [false, true] {
        for admitted in [false, true] {
            let mut fixture = Fixture::new()?;
            let key = "Extension.ValidationRule";
            fixture.node(key)?;
            fixture.link(
                key,
                CoreRelationshipTypeName::DescribedBy,
                "StringValidationRule.HolonType",
            )?;
            fixture.link("Contract", CoreRelationshipTypeName::ValidationBindings, key)?;
            let mut context = HolonValidationContext::resolve(&fixture.context)?;
            // Reuse two native roots that this string-only subject never dispatches.
            // Keep names unique and leave its actual holon/property/string rules intact.
            for (name, level, descriptor_family) in [
                (
                    CoreValidationRuleName::BaseValueKindMatchesInteger,
                    SubjectLevel::Property,
                    "HolonType.TypeDescriptor",
                ),
                (
                    CoreValidationRuleName::BaseValueKindMatchesBoolean,
                    SubjectLevel::Holon,
                    if admitted {
                        "HolonType.TypeDescriptor"
                    } else {
                        "PropertyType.TypeDescriptor"
                    },
                ),
            ] {
                let root = context
                    .values
                    .bindings
                    .entries
                    .iter_mut()
                    .find(|root| root.name == name)
                    .expect("fixture has canonical root");
                root.family = fixture.nodes["StringValidationRule.HolonType"].clone();
                root.level = level;
                root.descriptor_family = fixture.nodes[descriptor_family].clone();
            }
            if reverse {
                context.values.bindings.entries.reverse();
            }
            let mut subject = fixture.subject()?;
            subject.with_property_value("Title", "present")?;
            let mut collector = ValidationCollector::default();
            validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
            assert!(collector.observations().discovered_rule_keys.contains(key));
            assert!(!collector.observations().dispatched_rule_keys.contains(key));
            for name in [
                CoreValidationRuleName::NoUndescribedProperties,
                CoreValidationRuleName::RequiredPropertyPresence,
                CoreValidationRuleName::BaseValueKindMatchesString,
            ] {
                assert!(collector.observations().dispatched_rule_keys.contains(name.as_str()));
            }
            let report = collector.into_report();
            assert_eq!(report.violation_count(), 1);
            let findings = &report.violations;
            assert_eq!(findings[0].rule_key.as_deref(), Some(key));
            if admitted {
                assert_eq!(
                    findings[0].kind,
                    CommitValidationViolationKind::UnsupportedValidationRule
                );
            } else {
                assert!(matches!(&findings[0].kind,
                    CommitValidationViolationKind::RuleViolation { code }
                    if code == "IncompatibleValidationBinding"));
            }
        }
    }
    Ok(())
}

#[test]
fn binding_with_missing_or_ambiguous_describing_type_rejects_and_continues(
) -> Result<(), HolonError> {
    for ambiguous in [false, true] {
        let mut fixture = Fixture::new()?;
        let key = "Malformed.ValidationRule";
        fixture.node(key)?;
        if ambiguous {
            let families = vec![
                fixture.nodes["HolonValidationRule.HolonType"].clone(),
                fixture.nodes["PropertyValidationRule.HolonType"].clone(),
            ];
            // Author malformed input before a describing contract governs mutation.
            fixture
                .nodes
                .get_mut(key)
                .unwrap()
                .add_related_holons(CoreRelationshipTypeName::DescribedBy, families)?;
        }
        fixture.link("Contract", CoreRelationshipTypeName::ValidationBindings, key)?;
        let candidate = fixture.staged_subject("subject")?;
        let context = HolonValidationContext::resolve(&fixture.context)?;
        let mut collector = ValidationCollector::default();
        validate_holon(
            HolonValidationSubject { holon: &HolonReference::from(&candidate) },
            &context,
            &mut collector,
        )?;
        assert!(collector.observations().discovered_rule_keys.contains(key));
        assert!(!collector.observations().dispatched_rule_keys.contains(key));
        assert!(collector
            .observations()
            .dispatched_rule_keys
            .contains(CoreValidationRuleName::RequiredPropertyPresence.as_str()));
        let expected = collector.into_report();
        let report =
            validate_subject_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
        assert_eq!(report, expected);
        assert_eq!(report.violation_count(), 2);
        let finding = report
            .violations
            .iter()
            .find(|finding| finding.rule_key.as_deref() == Some(key))
            .unwrap();
        assert!(matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code }
            if code == "IncompatibleValidationBinding"));
        assert!(finding.message.contains(if ambiguous {
            "MultipleDescribedBy"
        } else {
            "MissingDescribedBy"
        }));
        assert!(finding.message.contains(&fixture.nodes[key].reference_id_string()));
        assert_eq!(
            finding.descriptor_identity,
            Some(fixture.nodes["Contract"].reference_id_string())
        );
        assert_eq!(candidate.validation_state()?, ValidationState::Invalid);
        assert_eq!(candidate.validation_findings()?, report.violations);
    }
    Ok(())
}

#[test]
fn unknown_rule_family_still_fails_closed_as_unsupported() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let key = "Extension.ValidationRule";
    fixture.node(key)?;
    fixture.node("ExtensionRuleFamily")?;
    fixture.link(key, CoreRelationshipTypeName::DescribedBy, "ExtensionRuleFamily")?;
    fixture.link("Contract", CoreRelationshipTypeName::ValidationBindings, key)?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut subject = fixture.subject()?;
    subject.with_property_value("Title", "present")?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert!(!collector.observations().dispatched_rule_keys.contains(key));
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert_eq!(report.violations[0].kind, CommitValidationViolationKind::UnsupportedValidationRule);
    assert_eq!(report.violations[0].rule_key.as_deref(), Some(key));
    Ok(())
}

#[test]
fn split_saved_schema_and_staged_binding_roots_are_compatible() -> Result<(), HolonError> {
    use crate::contexts::SubjectLevel;
    use crate::validators::compatible_binding_in_view;
    use holons_core::{ProspectiveDescriptorReader, ReadableHolon};
    let fixture = Fixture::new()?.saved_snapshot()?;
    // Earlier definitions are persisted; a later subject is staged independently.
    let mut subject = fixture.staged_subject("later-subject")?;
    subject.with_property_value("Title", "valid")?;
    assert!(validate_subject_candidates(&fixture.context, &[subject])?.is_accepted());

    let rule_key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let rule_update = fixture.replacement(rule_key)?;
    let family_update = fixture.replacement("HolonValidationRule.HolonType")?;
    let descriptor_update = fixture.replacement("HolonType.TypeDescriptor")?;
    let reader = ProspectiveDescriptorReader::new(
        &fixture.context,
        &[rule_update.clone(), family_update, descriptor_update],
    )?;
    let context = ValueValidationContext::resolve_in_view(&fixture.context, &reader).unwrap();
    let binding = ResolvedValidationBinding {
        rule: fixture.nodes[rule_key].clone(),
        declaring_descriptor: holons_core::HolonDescriptor::from_holon(
            fixture.nodes["HolonType.TypeDescriptor"].clone(),
        ),
    };
    assert!(compatible_binding_in_view(
        &binding,
        &fixture.nodes["Contract"],
        SubjectLevel::Holon,
        &context,
        &reader
    )
    .unwrap());
    assert!(!holons_core::same_definition(&binding.rule, &rule_update.clone().into()));
    assert!(holons_core::same_definition(
        &holons_core::DescriptorReader::select(&reader, &binding.rule).unwrap(),
        &rule_update.into()
    ));
    // Content selection must also apply to effective binding discovery, not just equality.
    let contributions = holons_core::descriptors::effective_relationship_targets_with_reader(
        &fixture.nodes["Contract"],
        CoreRelationshipTypeName::ValidationBindings,
        &reader,
    )
    .unwrap();
    assert_eq!(contributions.len(), 1);
    assert!(matches!(&contributions[0].member, HolonReference::Staged(_)));
    assert!(matches!(&contributions[0].declared_on, HolonReference::Staged(_)));
    assert_eq!(contributions[0].member.key()?.unwrap().to_string(), rule_key);
    Ok(())
}

#[test]
fn binding_family_and_constraint_type_use_replacement_content() -> Result<(), HolonError> {
    use holons_core::{
        EffectiveRelationshipMember, HolonCollection, HolonCollectionApi,
        ProspectiveDescriptorReader,
    };
    use std::sync::{Arc, RwLock};
    use type_names::ToRelationshipName;
    let fixture = Fixture::new()?.saved_snapshot()?;
    let rule_key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let wrong_family = fixture.nodes["PropertyValidationRule.HolonType"].clone();
    let update = fixture.replacement_with(rule_key, |model| {
        let mut members = HolonCollection::new_transient();
        members.add_references(vec![wrong_family.clone()])?;
        model.relationships.as_mut().unwrap().insert(
            CoreRelationshipTypeName::DescribedBy.to_relationship_name(),
            Arc::new(RwLock::new(members)),
        );
        Ok(())
    })?;
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &[update])?;
    let context = ValueValidationContext::resolve_in_view(&fixture.context, &reader).unwrap();
    let binding = ResolvedValidationBinding {
        rule: fixture.nodes[rule_key].clone(),
        declaring_descriptor: holons_core::HolonDescriptor::from_holon(
            fixture.nodes["HolonType.TypeDescriptor"].clone(),
        ),
    };
    assert!(!crate::validators::compatible_binding_in_view(
        &binding,
        &fixture.nodes["Contract"],
        crate::contexts::SubjectLevel::Holon,
        &context,
        &reader
    )
    .unwrap());
    let constraint = ResolvedConstraint::with_reader(
        EffectiveRelationshipMember {
            member: binding.rule,
            declared_on: fixture.nodes["HolonType.TypeDescriptor"].clone(),
        },
        &reader,
    )
    .unwrap();
    assert!(holons_core::same_definition(constraint.constraint_type.holon(), &wrong_family));
    Ok(())
}

#[test]
fn competition_diagnostics_are_deterministic_bounded_and_replaceable() -> Result<(), HolonError> {
    use holons_core::{AssessmentReadError, ProspectiveDescriptorReader};
    let fixture = Fixture::new()?.saved_snapshot()?;
    let key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let mut candidates = vec![fixture.replacement(key)?, fixture.replacement(key)?];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let first = competing_replacement_findings(&reader);
    assert_eq!(first.len(), 2);
    let context = ValueValidationContext::resolve(&fixture.context);
    // The old key lookup is deliberately not the prospective resolution path.
    assert!(matches!(context, Err(HolonError::DuplicateError(..))));
    let resolved = holons_core::descriptors::resolve_core_descriptor_with_reader(
        &fixture.context,
        key,
        &reader,
    );
    assert!(matches!(resolved, Err(AssessmentReadError::Contested { .. })));
    assert!(matches!(
        ValueValidationContext::resolve_in_view(&fixture.context, &reader),
        Err(AssessmentReadError::Contested { .. })
    ));
    let baseline_length = first.iter().map(|finding| finding.message.len()).max().unwrap();
    candidates.reverse();
    assert_eq!(
        first,
        competing_replacement_findings(&ProspectiveDescriptorReader::new(
            &fixture.context,
            &candidates
        )?)
    );
    for _ in 0..18 {
        candidates.push(fixture.replacement(key)?);
    }
    let findings = competing_replacement_findings(&ProspectiveDescriptorReader::new(
        &fixture.context,
        &candidates,
    )?);
    assert_eq!(findings.len(), candidates.len());
    let mut identities: Vec<_> =
        candidates.iter().map(|candidate| candidate.reference_id_string()).collect();
    identities.sort();
    for finding in &findings {
        assert!(
            matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "CompetingStagedReplacements")
        );
        assert!(finding.rule_key.is_none());
        assert!(finding.descriptor_identity.is_none());
        let ValidationSubjectPath::Holon { holon_identity } = &finding.subject else {
            panic!("candidate subject")
        };
        let other = if holon_identity == &identities[0] { &identities[1] } else { &identities[0] };
        assert!(finding.message.contains(holon_identity));
        assert!(finding.message.contains(other));
        assert!(finding.message.contains("20 live replacements"));
        assert!(finding
            .message
            .contains(&format!("{}", holons_core::ReadableHolon::holon_id(&fixture.nodes[key])?)));
        assert!(finding.message.len() <= baseline_length + 1, "only the count gains a digit");
    }
    // Commit orchestration prepares an assessment before installing outcomes. Its
    // two-phase carrier installs and replaces these per-candidate findings.
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    for candidate in &candidates {
        let report = CommitValidationReport::from_candidate(
            findings.iter().filter(|finding| matches!(&finding.subject, ValidationSubjectPath::Holon { holon_identity } if holon_identity == &candidate.reference_id_string())).cloned().collect(),
        );
        assessment.record_candidate(candidate, report);
    }
    assert!(!assessment.install_outcomes()?.is_accepted());
    for candidate in &candidates[1..] {
        candidate.abandon_staged_changes(&fixture.context)?;
    }
    let retry = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    assert!(competing_replacement_findings(&retry).is_empty());
    let mut assessment = crate::orchestration::PreparedAssessment::default();
    assessment.record_candidate(&candidates[0], CommitValidationReport::default());
    assert!(assessment.install_outcomes()?.is_accepted());
    Ok(())
}

#[test]
fn missing_new_validation_anchor_is_a_deliberate_schema_incompatibility() -> Result<(), HolonError>
{
    let mut fixture = Fixture::empty()?;
    let key = "AtMostOneDirectParent.ValidationRule";
    assert!(
        matches!(resolve_validation_anchor(&fixture.context, key), Err(holons_core::AssessmentReadError::SchemaIncompatible { missing_anchor }) if missing_anchor == key)
    );
    fixture.node(key)?;
    assert!(holons_core::same_definition(
        &resolve_validation_anchor(&fixture.context, key).unwrap(),
        &fixture.nodes[key]
    ));
    // Split bootstrap can find this anchor saved while later definitions are staged.
    let mut saved = fixture.saved_snapshot()?;
    saved.node("LaterDefinition")?;
    assert!(holons_core::same_definition(
        &resolve_validation_anchor(&saved.context, key).unwrap(),
        &saved.nodes[key]
    ));
    Ok(())
}

#[path = "schema_assessment_tests.rs"]
mod schema_assessment;
