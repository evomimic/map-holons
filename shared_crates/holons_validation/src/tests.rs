#[path = "test_support.rs"]
mod fixture;

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::core_shared_objects::{holon::ValidationState, Holon};
use holons_core::{Descriptor, HolonReference, PropertyDescriptor, ValueDescriptor, WritableHolon};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

use super::*;
use fixture::{Fixture, RULES};

fn property_path(name: &str) -> ValidationSubjectPath {
    ValidationSubjectPath::Property { holon_identity: "subject".into(), name: name.into() }
}

fn value_path() -> ValidationSubjectPath {
    ValidationSubjectPath::Value { holon_identity: "subject".into(), property: "Title".into() }
}

#[test]
fn commit_candidates_replace_all_outcomes_and_accept_corrected_retry() -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let mut missing_title = fixture.staged_subject("missing-title")?;
    let mut clean = fixture.staged_subject("clean")?;
    clean.with_property_value("Title", "present")?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("undescribed".into())))?;
    let mut undescribed = fixture.context.mutation().stage_new_holon(transient)?;

    // Seed outcomes contrary to the authored inputs: no prior state may skip reassessment.
    missing_title.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    let stale_findings =
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&undescribed))?
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
    let report = validate_commit_candidates(&fixture.context, &candidates)?;
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
    let report = validate_commit_candidates(&fixture.context, &candidates)?;
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
fn commit_candidate_assessment_error_installs_no_partial_outcomes() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("first".into())))?;
    let first = fixture.context.mutation().stage_new_holon(transient)?;
    let failing = fixture.staged_subject("failing")?;
    let stale_findings =
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&first))?.violations;
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

    assert_eq!(validate_commit_candidates(&fixture.context, &candidates), Err(expected_error));
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
fn terminal_commit_candidate_is_refused_before_any_outcome_installation() -> Result<(), HolonError>
{
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
            validate_commit_candidates(&fixture.context, &candidates),
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
fn commit_candidate_anchor_resolution_error_preserves_prior_outcome() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("subject".into())))?;
    let candidate = fixture.context.mutation().stage_new_holon(transient)?;
    candidate.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    assert!(matches!(
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate)),
        Err(HolonError::HolonNotFound(_))
    ));
    assert_eq!(candidate.validation_state()?, ValidationState::Validated);
    assert!(candidate.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn empty_commit_candidates_are_accepted_without_schema_anchors() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let report = validate_commit_candidates(&fixture.context, &[])?;
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
fn undescribed_property_policy_rejects_and_inherits_permission() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
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
    drop(context);
    fixture
        .nodes
        .get_mut("HolonType.TypeDescriptor")
        .unwrap()
        .with_property_value("AllowsAdditionalProperties", true)?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let mut collector = ValidationCollector::default();
    validate_holon(HolonValidationSubject { holon: &subject }, &context, &mut collector)?;
    assert!(collector.into_report().is_accepted());
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
fn incomplete_descriptor_read_returns_error_instead_of_semantic_acceptance(
) -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    let context = HolonValidationContext::resolve(&fixture.context)?;
    let subject = fixture.subject()?;
    assert!(validate_holon(
        HolonValidationSubject { holon: &subject },
        &context,
        &mut ValidationCollector::default()
    )
    .is_err());
    Ok(())
}

#[test]
fn registry_covers_exactly_the_authored_cohort_and_report_rejects_every_finding() {
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
    fixture
        .nodes
        .get_mut("Contract")
        .unwrap()
        .with_property_value("AllowsAdditionalProperties", true)?;
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
    assert!(matches!(
        validate_value(
            ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
            &context,
            &mut ValidationCollector::default(),
        ),
        Err(HolonError::MissingDescribedBy { .. })
    ));
    Ok(())
}

#[test]
fn incomplete_schema_prevents_constructing_a_validation_context() -> Result<(), HolonError> {
    let mut fixture = Fixture::empty()?;
    // Core descriptor roots alone are insufficient: rule anchors must also be loaded.
    for key in [
        "TypeDescriptor",
        "MetaTypeDescriptor.HolonType",
        "StringValueType.ValueType",
        "IntegerValueType.ValueType",
        "BooleanValueType.ValueType",
        "BytesValueType.ValueType",
        "EnumValueType.ValueType",
        "BaseValueValueType.ValueType",
        "ValueArrayValueType.ValueType",
    ] {
        fixture.node(key)?;
    }
    assert!(matches!(
        HolonValidationContext::resolve(&fixture.context),
        Err(HolonError::HolonNotFound(_))
    ));
    Ok(())
}
