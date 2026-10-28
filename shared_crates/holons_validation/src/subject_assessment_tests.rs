#[path = "key_uniqueness_tests.rs"]
mod key_uniqueness_tests;

#[path = "graph_only_deltas_tests.rs"]
mod graph_only_deltas_tests;

#[path = "core_successor_tests.rs"]
mod core_successors;

#[path = "constraint_declarations_tests.rs"]
mod constraint_declarations;

#[path = "test_support.rs"]
mod fixture;

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::core_shared_objects::{holon::ValidationState, Holon};
use holons_core::{Descriptor, HolonReference, ReadableHolon, ValueDescriptor, WritableHolon};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

use super::*;
use fixture::{c2_kind_semantics, readiness_fixture, subject_rules, Fixture};

/// Selects the Title member's value contract while retaining exactly one ValueType edge.
fn select_title_value_type(fixture: &mut Fixture, key: &str) -> Result<(), HolonError> {
    let property = fixture.nodes.get_mut("Title.PropertyType").unwrap();
    let old = property.related_holons(CoreRelationshipTypeName::ValueType)?;
    let members = old.read().unwrap().get_members().clone();
    property.remove_related_holons(CoreRelationshipTypeName::ValueType, members)?;
    fixture.link("Title.PropertyType", CoreRelationshipTypeName::ValueType, key)
}

/// Builds an abstract descriptor's meta-contract, including its authored structural edges.
fn declare_abstract_property(fixture: &mut Fixture) -> Result<(), HolonError> {
    fixture.link(
        "HolonType.TypeDescriptor",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    fixture.node("AbstractMetaType")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "AbstractMetaType",
        CoreRelationshipTypeName::Extends,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    for name in ["IsAbstractType", "DefinesInstanceTypeKind"] {
        let key = format!("{name}.PropertyType");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(&key, CoreRelationshipTypeName::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(&key, CoreRelationshipTypeName::ValueType, "BooleanValueType.ValueType")?;
        fixture.link("AbstractMetaType", CoreRelationshipTypeName::InstanceProperties, &key)?;
    }
    for key in ["Title.PropertyType", "Key.PropertyType"] {
        fixture.link("AbstractMetaType", CoreRelationshipTypeName::InstanceProperties, key)?;
    }
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.node("AbstractSchema")?;
    fixture.link("AbstractSchema", CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    for (name, target) in [("Extends", "TypeDescriptor"), ("ComponentOf", "Schema.HolonType")] {
        let key = format!("{name}.AbstractRelationship");
        fixture
            .node(&key)?
            .with_property_value("TypeName", name)?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(
            &key,
            CoreRelationshipTypeName::Extends,
            "DeclaredRelationshipType.RelationshipType",
        )?;
        fixture.link(&key, CoreRelationshipTypeName::SourceType, "AbstractMetaType")?;
        fixture.link(&key, CoreRelationshipTypeName::TargetType, target)?;
        fixture.link("AbstractMetaType", CoreRelationshipTypeName::InstanceRelationships, &key)?;
    }
    Ok(())
}

fn abstract_subject(fixture: &Fixture) -> Result<holons_core::StagedReference, HolonError> {
    let mut input = fixture.context.mutation().new_holon(Some("abstract-subject".into()))?;
    input.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![fixture.nodes["HolonType.TypeDescriptor"].clone()],
    )?;
    input.add_related_holons(
        CoreRelationshipTypeName::ComponentOf,
        vec![fixture.nodes["AbstractSchema"].clone()],
    )?;
    let mut subject = fixture.context.mutation().stage_new_holon(input)?;
    subject.with_descriptor(fixture.nodes["AbstractMetaType"].clone())?;
    subject
        .with_property_value("IsAbstractType", true)?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    Ok(subject)
}

/// Checks a selected binding against role facts prepared from its exact references.
fn compatible_binding_in_view(
    binding: &ResolvedValidationBinding,
    governing: &HolonReference,
    level: crate::contexts::SubjectLevel,
    context: &ValueValidationContext,
) -> Result<bool, HolonError> {
    use holons_core::{
        DescribingTypeResolution, EffectiveRelationshipMember, ExtendsLineageDiagnosis,
    };
    let key = crate::commitments::required_key(&binding.rule)?;
    let Some(registration) = CoreValidationRuleName::from_key(&key)
        .map(crate::registries::registration)
        .filter(|registration| registration.implementation.is_some())
    else {
        return Ok(false);
    };
    let DescribingTypeResolution::Unique(family) =
        holons_core::resolve_describing_type(&binding.rule)?
    else {
        return Ok(false);
    };
    let contribution = EffectiveRelationshipMember {
        member: binding.rule.clone(),
        declared_on: binding.declaring_descriptor.holon().clone(),
    };
    let diagnosis = ExtendsLineageDiagnosis::assess(governing)?;
    let roles = crate::subject_assessment::BindingRoleMemo::prepare(
        &context.context,
        [&contribution],
        diagnosis.valid_lineage(),
    )?;
    crate::subject_assessment::compatible_binding(
        binding,
        &family,
        registration,
        governing,
        level,
        context,
        &roles,
    )
}

fn value_path() -> ValidationSubjectPath {
    ValidationSubjectPath::Value { holon_identity: "subject".into(), property: "Title".into() }
}

#[test]
fn subject_gate_replaces_all_outcomes_and_accepts_corrected_retry() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
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
fn subject_gate_rejects_removed_attachment_default_without_refilling() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .with_property_value("DefaultValue", "default title")?;
    let mut subject = fixture.staged_subject("removed-default")?;
    assert_eq!(
        subject.property_value("Title")?,
        Some(BaseValue::StringValue(MapString("default title".into())))
    );
    subject.remove_property_value("Title")?;
    // A prior successful assessment never suppresses the fresh assessment.
    subject.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!report.is_accepted());
    assert!(report.violations.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation {code} if code == "DS-PROP-001")));
    assert_eq!(subject.validation_state()?, ValidationState::Invalid);
    assert_eq!(subject.property_value("Title")?, None);
    Ok(())
}

#[test]
fn subject_gate_assessment_error_installs_no_partial_outcomes() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let transient = fixture.context.mutation().new_holon(Some("first".into()))?;
    let first = fixture.context.mutation().stage_new_holon(transient)?;
    let failing = fixture.staged_subject("failing")?;
    let stale_findings =
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&first))?.violations;
    first.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    failing.replace_validation_outcome(ValidationState::Invalid, stale_findings)?;
    let foreign = Fixture::new()?;
    fixture.nodes.get_mut("Contract").unwrap().add_related_holons(
        CoreRelationshipTypeName::InstanceProperties,
        vec![foreign.nodes["Title.PropertyType"].clone()],
    )?;
    let candidates = [first, failing];
    let before = candidates
        .iter()
        .map(|candidate| {
            Ok(candidate.get_holon_to_commit(&fixture.context)?.read().unwrap().clone())
        })
        .collect::<Result<Vec<_>, HolonError>>()?;
    let assessment_error = assess_commit_candidates(&fixture.context, &candidates).unwrap_err();
    let validation_error = validate_commit_candidates(&fixture.context, &candidates).unwrap_err();
    assert!(matches!(assessment_error, HolonError::CrossTransactionReference { .. }));
    assert_eq!(assessment_error, validation_error);
    for (candidate, before) in candidates.iter().zip(before) {
        assert_eq!(*candidate.get_holon_to_commit(&fixture.context)?.read().unwrap(), before);
    }
    Ok(())
}

#[test]
fn terminal_candidate_is_refused_before_any_outcome_installation() -> Result<(), HolonError> {
    for committed in [false, true] {
        let fixture = readiness_fixture()?;
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
fn absent_core_reports_the_encountered_missing_descriptor() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let transient = fixture.context.mutation().new_holon(Some(MapString("subject".into())))?;
    let candidate = fixture.context.mutation().stage_new_holon(transient)?;
    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
    assert!(report.violations.iter().any(|finding| {
        finding.kind == CommitValidationViolationKind::NoDescriptor
            && finding.message.contains("MissingDescribedBy")
    }));
    assert!(!report.is_accepted());
    assert_eq!(candidate.validation_state()?, ValidationState::NoDescriptor);
    Ok(())
}

#[test]
fn empty_subject_workset_is_accepted_without_schema_anchors() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let report = validate_commit_candidates(&fixture.context, &[])?;
    assert!(report.is_accepted());
    assert_eq!(report.violation_count(), 0);
    Ok(())
}

#[test]
fn all_five_native_handlers_accept_and_reject_without_configured_evaluation(
) -> Result<(), HolonError> {
    let values = [
        BaseValue::StringValue(MapString("text".into())),
        BaseValue::IntegerValue(MapInteger(42)),
        BaseValue::BooleanValue(MapBoolean(true)),
        BaseValue::BytesValue(MapBytes(vec![1, 2])),
        BaseValue::EnumValue(MapEnumValue(MapString("member".into()))),
    ];
    for (index, (rule, _, target)) in subject_rules()
        .filter(|(name, _, _)| crate::registries::registration(*name).native_kind.is_some())
        .enumerate()
    {
        let mut fixture = readiness_fixture()?;
        select_title_value_type(&mut fixture, target)?;
        let mut subject = fixture.staged_subject("native-value")?;
        for (value_index, value) in values.iter().enumerate() {
            subject.with_property_value("Title", value.clone())?;
            let assessment =
                assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
            assert!(assessment.observations.dispatched_rule_keys.contains(rule.as_str()));
            assert_eq!(
                assessment.report.is_accepted(),
                index == value_index,
                "{:?}",
                assessment.report
            );
            assert_eq!(assessment.report.violation_count(), usize::from(index != value_index));
        }
    }
    Ok(())
}

#[test]
fn bytes_binding_is_discovered_dispatched_and_rejects_a_mismatched_value_kind(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    select_title_value_type(&mut fixture, "BytesValueType.ValueType")?;
    let mut subject = fixture.staged_subject("bytes-probe")?;
    subject.with_property_value("Title", BaseValue::BytesValue(MapBytes(vec![0, 1, 127, 255])))?;
    let candidates = std::slice::from_ref(&subject);
    let accepted = assess_commit_candidates(&fixture.context, candidates)?;
    let rule = CoreValidationRuleName::BaseValueKindMatchesBytes.as_str();
    assert!(accepted.observations.discovered_rule_keys.contains(rule));
    assert!(accepted.observations.dispatched_rule_keys.contains(rule));
    assert_eq!(accepted.observations.effective_constraint_count, 0);
    assert!(accepted.report.is_accepted(), "{:?}", accepted.report);

    subject.with_property_value("Title", 42_i64)?;
    let candidates = std::slice::from_ref(&subject);
    let rejected = assess_commit_candidates(&fixture.context, candidates)?;
    assert!(rejected.observations.discovered_rule_keys.contains(rule));
    assert!(rejected.observations.dispatched_rule_keys.contains(rule));
    assert_eq!(rejected.report.violation_count(), 1);
    let finding = &rejected.report.violations[0];
    assert!(matches!(&finding.kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "BaseValueKindMismatch"));
    assert_eq!(finding.rule_key.as_deref(), Some(rule));
    assert_eq!(
        finding.subject,
        ValidationSubjectPath::Value {
            holon_identity: subject.full_reference_id_string(),
            property: "Title".into(),
        }
    );
    assert_eq!(
        finding.descriptor_identity,
        Some(fixture.nodes["BytesValueType.ValueType"].reference_id_string())
    );
    assert_eq!(validate_commit_candidates(&fixture.context, candidates)?, rejected.report);
    assert_eq!(subject.validation_state()?, ValidationState::Invalid);
    Ok(())
}

#[test]
fn full_contract_traversal_assesses_absent_required_properties() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("subject")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    let report = assessment.report;
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
    );
    assert!(
        matches!(&report.violations[0].subject, ValidationSubjectPath::Property { name, .. } if name == "Title")
    );
    subject.with_property_value("Title", "present")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert_eq!(assessment.observations.effective_constraint_count, 0);
    assert!(assessment.report.is_accepted());
    Ok(())
}

#[test]
fn property_minimum_exemption_does_not_skip_populated_value_validation() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let mut subject = abstract_subject(&fixture)?;
    let report = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?.report;
    assert!(report.is_accepted(), "{:?}", report);
    subject.with_property_value("Title", 3_i64)?;
    let report = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?.report;
    assert!(!report.is_accepted());
    assert!(report.violations.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "BaseValueKindMismatch")));
    Ok(())
}

#[test]
fn undescribed_property_policy_always_rejects() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("subject")?;
    subject.with_property_value("Title", "present")?.with_property_value("Extra", true)?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    let report = assessment.report;
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-003")
    );
    Ok(())
}

#[test]
fn missing_and_multiple_descriptors_stop_bootstrap_with_distinct_findings() -> Result<(), HolonError>
{
    let fixture = readiness_fixture()?;
    let transient = fixture.context.mutation().new_holon(Some("undescribed".into()))?;
    let mut subject = fixture.context.mutation().stage_new_holon(transient)?;
    let mut messages = Vec::new();
    for multiple in [false, true] {
        if multiple {
            subject.add_related_holons(
                CoreRelationshipTypeName::DescribedBy,
                vec![fixture.nodes["Contract"].clone(), fixture.nodes["TypeDescriptor"].clone()],
            )?;
        }
        let assessment =
            assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
        assert!(assessment.observations.dispatched_rule_keys.is_empty());
        assert_eq!(assessment.report.violation_count(), 1);
        assert_eq!(
            assessment.report.violations[0].kind,
            CommitValidationViolationKind::NoDescriptor
        );
        messages.push(assessment.report.violations[0].message.clone());
    }
    assert!(messages[0].starts_with("MissingDescribedBy:"));
    assert!(messages[1].starts_with("MultipleDescribedBy:"));
    Ok(())
}

#[test]
fn incompatible_binding_is_rejected_before_its_handler_dispatch() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let key = CoreValidationRuleName::BaseValueKindMatchesString.as_str();
    fixture.link("Title.PropertyType", CoreRelationshipTypeName::ValidationBindings, key)?;
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .with_property_value("IsValueRequired", false)?;
    let mut subject = fixture.staged_subject("incompatible")?;
    subject.remove_property_value("Key")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!assessment.observations.dispatched_rule_keys.contains(key));
    let report = assessment.report;
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "IncompatibleValidationBinding")
    );
    assert_eq!(
        report.violations[0].descriptor_identity,
        Some(fixture.nodes["Title.PropertyType"].reference_id_string())
    );
    Ok(())
}

#[test]
fn unsupported_rules_and_constraints_fail_closed_with_contribution_provenance(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
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
    select_title_value_type(&mut fixture, "StringValueType.ValueType")?;
    let descriptor =
        ValueDescriptor::from_holon(fixture.nodes["StringValueType.ValueType"].clone());
    let mut subject = fixture.staged_subject("value-subject")?;
    subject.with_property_value("Title", BaseValue::IntegerValue(MapInteger(5)))?;
    subject.remove_property_value("Key")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert_eq!(assessment.observations.effective_constraint_count, 1);
    let report = assessment.report;
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
fn subject_rule_registrations_cover_fixture_bindings_and_report_rejects_findings() {
    for (rule, _, _) in subject_rules() {
        assert!(crate::registries::registration(rule).implementation.is_some());
    }
    assert!(CoreValidationRuleName::from_key("Unknown.ValidationRule").is_none());
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
    let mut fixture = readiness_fixture()?;
    fixture.node("Unbound.ValueType")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Unbound.ValueType",
        CoreRelationshipTypeName::Extends,
        "IntegerValueType.ValueType",
    )?;
    let rule = fixture.nodes[CoreValidationRuleName::BaseValueKindMatchesInteger.as_str()].clone();
    fixture
        .nodes
        .get_mut("IntegerValueType.ValueType")
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::ValidationBindings, vec![rule])?;
    select_title_value_type(&mut fixture, "Unbound.ValueType")?;
    let mut subject = fixture.staged_subject("unbound-value")?;
    subject.with_property_value("Title", 3_i64)?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    let key = CoreValidationRuleName::BaseValueKindMatchesInteger.as_str();
    assert!(!assessment.observations.discovered_rule_keys.contains(key));
    assert!(!assessment.observations.dispatched_rule_keys.contains(key));
    assert!(assessment.report.is_accepted(), "{:?}", assessment.report);
    subject.remove_property_value("Key")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(assessment
        .observations
        .dispatched_rule_keys
        .contains(CoreValidationRuleName::RequiredPropertyPresence.as_str()));
    assert!(assessment.report.is_accepted());
    Ok(())
}

#[test]
fn abstract_descriptor_exemption_is_computed_from_the_universal_contract() -> Result<(), HolonError>
{
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let subject = abstract_subject(&fixture)?;
    let report = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?.report;
    assert!(report.is_accepted(), "{:?}", report);
    let title = fixture.nodes["Title.PropertyType"].clone();
    fixture
        .nodes
        .get_mut("AbstractMetaType")
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::InstanceProperties, vec![title])?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::InstanceProperties,
        "Title.PropertyType",
    )?;
    let report = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?.report;
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
    );
    Ok(())
}

#[test]
fn binding_compatibility_uses_rule_type_lineage_and_original_declaration() -> Result<(), HolonError>
{
    let mut fixture = readiness_fixture()?;
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
    fixture
        .node("Derived.StringValueType")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Derived.StringValueType",
        CoreRelationshipTypeName::Extends,
        "StringValueType.ValueType",
    )?;
    select_title_value_type(&mut fixture, "Derived.StringValueType")?;
    let mut subject = fixture.staged_subject("value-subject")?;
    subject.with_property_value("Title", BaseValue::BooleanValue(MapBoolean(true)))?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(assessment.observations.dispatched_rule_keys.contains(rule_key));
    let report = assessment.report;
    assert_eq!(report.violation_count(), 1);
    assert_eq!(
        report.violations[0].descriptor_identity,
        Some(fixture.nodes["StringValueType.ValueType"].reference_id_string())
    );
    Ok(())
}

#[test]
fn canonical_rule_with_wrong_authored_family_never_dispatches() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
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
    select_title_value_type(&mut fixture, "StringValueType.ValueType")?;
    let mut subject = fixture.staged_subject("value-subject")?;
    subject.with_property_value("Title", BaseValue::StringValue(MapString("text".into())))?;
    subject.remove_property_value("Key")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!assessment.observations.dispatched_rule_keys.contains(rule_key));
    assert_eq!(assessment.report.violation_count(), 1);
    Ok(())
}

#[test]
fn unsupported_constraints_on_holon_and_absent_property_are_not_skipped() -> Result<(), HolonError>
{
    let mut fixture = readiness_fixture()?;
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
    let subject = fixture.staged_subject("subject")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert_eq!(assessment.observations.effective_constraint_count, 3);
    let report = assessment.report;
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
fn unresolved_constraint_type_is_an_incomplete_assessment() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("Undescribed.Constraint")?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "Undescribed.Constraint",
    )?;
    let mut subject = fixture.staged_subject("constraint-subject")?;
    subject.with_property_value("Title", "present")?;
    assert!(matches!(
        assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject)),
        Err(HolonError::MissingDescribedBy { .. })
    ));
    Ok(())
}

#[test]
fn unrelated_missing_rule_does_not_block_validation() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("unrelated-missing-rule")?;
    subject.with_property_value("Title", "present")?;
    let rule =
        fixture.nodes.remove(CoreValidationRuleName::SchemaDependenciesAcyclic.as_str()).unwrap();
    let HolonReference::Staged(rule) = rule else { unreachable!() };
    rule.abandon_staged_changes(&fixture.context)?;
    assert!(assess_commit_candidates(&fixture.context, &[subject])?.report.is_accepted());
    Ok(())
}

#[test]
fn subject_gate_rejects_unlicensed_relationships_assembled_before_descriptors(
) -> Result<(), HolonError> {
    for name in ["AuthorOf", "UnknownRelationship"] {
        let fixture = readiness_fixture()?;
        let mut clean = fixture.staged_subject("clean-peer")?;
        clean.with_property_value("Title", "clean")?;
        let mut input = fixture.context.mutation().new_holon(Some("raw-input".into()))?;
        input.add_related_holons(name, vec![clean.clone().into()])?;
        let mut candidate = fixture.context.mutation().stage_new_holon(input)?;
        candidate.with_descriptor(fixture.nodes["Contract"].clone())?;
        candidate.with_property_value("Title", "invalid relationship")?;
        let candidates = [clean.clone(), candidate.clone()];
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
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
        assert!(validate_commit_candidates(&fixture.context, &candidates)?.is_accepted());
        assert!(candidate.validation_findings()?.is_empty());
    }
    Ok(())
}

#[test]
fn subject_gate_accepts_inherited_declarations_without_target_descriptors() -> Result<(), HolonError>
{
    let mut fixture = readiness_fixture()?;
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
    fixture
        .nodes
        .get_mut("Forward.Relationship")
        .unwrap()
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Forward.Relationship",
        CoreRelationshipTypeName::SourceType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "Forward.Relationship",
        CoreRelationshipTypeName::TargetType,
        "HolonType.TypeDescriptor",
    )?;
    let mut candidate = fixture.staged_subject("candidate")?;
    candidate.with_property_value("Title", "declared source")?;
    // No target descriptor exists. Declared-write authorization is source-only.
    let target = fixture.node("undescribed-target")?;
    candidate.add_related_holons_ungoverned("Forward", vec![target])?;
    assert!(validate_commit_candidates(&fixture.context, &[candidate])?.is_accepted());
    Ok(())
}

#[test]
fn malformed_member_contract_reports_findings_and_installs_outcomes() -> Result<(), HolonError> {
    for member in ["DescribedBy.Relationship", "Title.PropertyType"] {
        let mut fixture = readiness_fixture()?;
        let missing = fixture.context.mutation().new_holon(Some("undescribed".into()))?;
        let first = fixture.context.mutation().stage_new_holon(missing)?;
        first.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
        let candidate = fixture.staged_subject("candidate")?;
        fixture.nodes.get_mut(member).unwrap().remove_property_value("TypeName")?;
        let candidates = [first.clone(), candidate.clone()];
        let assessment = assess_commit_candidates(&fixture.context, &candidates)?;
        assert!(!assessment.report.is_accepted());
        assert_eq!(first.validation_state()?, ValidationState::Validated);
        assert!(first.validation_findings()?.is_empty());
        assert_eq!(candidate.validation_state()?, ValidationState::ValidationRequired);
        assert!(candidate.validation_findings()?.is_empty());
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert_eq!(report, assessment.report);
        assert_eq!(first.validation_state()?, ValidationState::NoDescriptor);
        assert_eq!(candidate.validation_state()?, ValidationState::Invalid);
        assert!(candidate.validation_findings()?.iter().any(
            |finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency
        ));
        assert!(report.unattached_findings()?.iter().any(|finding| finding.subject
            == ValidationSubjectPath::Holon {
                holon_identity: fixture.nodes["Contract"].full_reference_id_string()
            }
            && finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    }
    Ok(())
}

#[test]
fn aggregate_only_report_rejects_without_installing_staged_outcomes() -> Result<(), HolonError> {
    let mut assessment = crate::outcomes::PreparedAssessment::default();
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
    let fixture = readiness_fixture()?;
    let invalid = fixture.staged_subject("invalid")?;
    let clean = fixture.staged_subject("clean")?;
    let candidate_report =
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&invalid))?;
    let mut aggregate = candidate_report.violations[0].clone();
    aggregate.subject = ValidationSubjectPath::Holon { holon_identity: "unstaged-schema".into() };
    let mut assessment = crate::outcomes::PreparedAssessment::default();
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
    let fixture = readiness_fixture()?;
    let candidate = fixture.staged_subject("candidate")?;
    let candidate_report =
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
    let finding = candidate_report.violations[0].clone();
    let mut assessment = crate::outcomes::PreparedAssessment::default();
    assessment.record_candidate(&candidate, candidate_report);
    assessment.push_aggregate(finding.clone());

    let report = assessment.install_outcomes()?;
    assert_eq!(report.violations, vec![finding.clone(), finding.clone()]);
    assert_eq!(candidate.validation_findings()?, vec![finding.clone()]);
    assert_eq!(report.unattached_findings()?, vec![&finding]);
    Ok(())
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
    let path = ValidationSubjectPath::Holon { holon_identity: subject.full_reference_id_string() };
    let mut collector = ValidationCollector::default();
    let registration = crate::registries::registration(rule);
    assert!(matches!(
        registration.implementation,
        Some(crate::registries::RuleImplementation::DescriptorProduct)
    ));
    products.emit(rule, registration.code, &binding, &path, &mut collector)?;
    Ok(collector.into_report().violations)
}

#[test]
fn descriptor_rules_have_product_routes_and_no_native_value_kind() {
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
        let registration = crate::registries::registration(rule);
        assert!(matches!(
            registration.implementation,
            Some(crate::registries::RuleImplementation::DescriptorProduct)
        ));
        assert_eq!(registration.native_kind, None);
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
    let prerequisites = StructuralPrerequisites::assess(&subject)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_structure(&prerequisites);
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
        let prerequisites = StructuralPrerequisites::assess(&subject)?;
        let mut products = DescriptorRuleProducts::default();
        products.prepare_structure(&prerequisites);
        let findings = dispatch_c2_rule(&mut fixture, rule, &products, &subject)?;
        assert!(findings.iter().any(|finding| matches!(&finding.kind,
            CommitValidationViolationKind::RuleViolation { code: actual } if actual == code)));
    }
    Ok(())
}

#[test]
fn rootless_holon_is_not_inferred_to_be_a_descriptor_from_its_meta_type() -> Result<(), HolonError>
{
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
    fixture.node("Rootless")?;
    fixture.link(
        "Rootless",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let subject = fixture.nodes["Rootless"].clone();
    let prerequisites = StructuralPrerequisites::assess(&subject)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_structure(&prerequisites);
    products.prepare_kind(&prerequisites, &roots)?;
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
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
    let subject = fixture.nodes["Contract"].clone();
    let prerequisites = StructuralPrerequisites::assess(&subject)?;
    fixture
        .nodes
        .get_mut("Contract")
        .unwrap()
        .remove_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind)?;
    let mut missing = DescriptorRuleProducts::default();
    missing.prepare_kind(&prerequisites, &roots)?;
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::LocalInstanceKindAnchorDesignation,
        &missing,
        &subject,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(findings.iter().any(|finding| matches!(
        &finding.kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-001"
    )));
    fixture
        .nodes
        .get_mut("Contract")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?;
    let mut non_abstract = DescriptorRuleProducts::default();
    non_abstract.prepare_kind(&prerequisites, &roots)?;
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::InstanceKindAnchorsAreAbstract,
        &non_abstract,
        &subject,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(findings.iter().any(|finding| matches!(
        &finding.kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-002"
    )));
    Ok(())
}

#[test]
fn kind_handlers_reject_an_ordinary_holon_with_an_incompatible_describer() -> Result<(), HolonError>
{
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
    let mut subject = fixture.context.mutation().new_holon(Some("ordinary".into()))?;
    subject.with_descriptor(fixture.nodes["StringValueType.ValueType"].clone())?;
    let subject: HolonReference = subject.into();
    let prerequisites = StructuralPrerequisites::assess(&subject)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &roots)?;
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
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
    let meta_holon = fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone();
    fixture
        .nodes
        .get_mut("MetaHolonType.MetaTypeDescriptor")
        .unwrap()
        .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![meta_holon.clone()])?;
    let prerequisites = StructuralPrerequisites::assess(&meta_holon)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &roots)?;
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescriptorMetaTypeCorrespondence,
        &products,
        &meta_holon,
    )?
    .is_empty());

    let root = fixture.nodes["TypeDescriptor"].clone();
    let root_prerequisites = StructuralPrerequisites::assess(&root)?;
    let mut root_products = DescriptorRuleProducts::default();
    root_products.prepare_kind(&root_prerequisites, &roots)?;
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
    invalid_root_products.prepare_kind(&root_prerequisites, &roots)?;
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::TypeDescriptorRootKindException,
        &invalid_root_products,
        &root,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(findings.iter().any(|finding| matches!(
        &finding.kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-003"
    )));
    Ok(())
}

#[test]
fn contract_handlers_keep_redeclaration_name_and_definition_defects_distinct(
) -> Result<(), HolonError> {
    use holons_core::ContractContributions;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
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
    let mut products = DescriptorRuleProducts::default();
    products.prepare_contract(&contributions, &roots)?;
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
    use holons_core::ContractContributions;
    let mut fixture = Fixture::new()?;
    let roots = c2_kind_semantics(&mut fixture)?;
    fixture
        .nodes
        .get_mut("StringValueType.ValueType")
        .unwrap()
        .with_property_value(type_names::CorePropertyTypeName::DefinesInstanceTypeKind, true)?
        .with_property_value(type_names::CorePropertyTypeName::IsAbstractType, true)?;
    fixture.node("ValueType.TypeDescriptor")?;
    fixture.link(
        "ValueType.TypeDescriptor",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    let root = fixture.nodes["TypeDescriptor"].clone();
    fixture
        .nodes
        .get_mut("StringValueType.ValueType")
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::Extends, vec![root])?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Extends,
        "ValueType.TypeDescriptor",
    )?;
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
    products.prepare_contract(&contributions, &roots)?;
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
    let mut assessment = crate::outcomes::PreparedAssessment::default();
    assessment.push_aggregate(finding);
    let mut report = assessment.install_outcomes()?;
    report.violations.clear();
    assert!(matches!(report.unattached_findings(), Err(HolonError::CommitFailure(_))));
    Ok(())
}

#[test]
fn unknown_bindings_fail_closed_regardless_of_subject_level() -> Result<(), HolonError> {
    for admitted in [false, true] {
        let mut fixture = readiness_fixture()?;
        let key = "Extension.ValidationRule";
        fixture.link(
            "PropertyValidationRule.HolonType",
            CoreRelationshipTypeName::Extends,
            "HolonValidationRule.HolonType",
        )?;
        fixture.node(key)?;
        fixture.link(
            key,
            CoreRelationshipTypeName::DescribedBy,
            "PropertyValidationRule.HolonType",
        )?;
        fixture.link(
            if admitted { "Contract" } else { "StringValueType.ValueType" },
            CoreRelationshipTypeName::ValidationBindings,
            key,
        )?;
        let mut subject = fixture.staged_subject("extension-subject")?;
        subject.with_property_value("Title", "present")?;
        subject.remove_property_value("Key")?;
        let assessment =
            assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
        assert!(assessment.observations.discovered_rule_keys.contains(key));
        assert!(!assessment.observations.dispatched_rule_keys.contains(key));
        for name in [
            CoreValidationRuleName::NoUndescribedProperties,
            CoreValidationRuleName::RequiredPropertyPresence,
            CoreValidationRuleName::BaseValueKindMatchesString,
        ] {
            assert!(assessment.observations.dispatched_rule_keys.contains(name.as_str()));
        }
        assert_eq!(assessment.report.violation_count(), 1);
        let finding = &assessment.report.violations[0];
        assert_eq!(finding.rule_key.as_deref(), Some(key));
        assert_eq!(finding.kind, CommitValidationViolationKind::UnsupportedValidationRule);
    }
    Ok(())
}

#[test]
fn unknown_binding_with_missing_or_ambiguous_describer_is_unsupported() -> Result<(), HolonError> {
    for ambiguous in [false, true] {
        let mut fixture = readiness_fixture()?;
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
        let assessment =
            assess_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
        assert!(assessment.observations.discovered_rule_keys.contains(key));
        assert!(!assessment.observations.dispatched_rule_keys.contains(key));
        assert!(assessment
            .observations
            .dispatched_rule_keys
            .contains(CoreValidationRuleName::RequiredPropertyPresence.as_str()));
        let expected = assessment.report;
        let report =
            validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
        assert_eq!(report, expected);
        assert_eq!(report.violation_count(), 2);
        let finding = report
            .violations
            .iter()
            .find(|finding| finding.rule_key.as_deref() == Some(key))
            .unwrap();
        assert_eq!(finding.kind, CommitValidationViolationKind::UnsupportedValidationRule);
        assert!(finding.message.contains("no registered handler"));
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
    let mut fixture = readiness_fixture()?;
    let key = "Extension.ValidationRule";
    fixture.node(key)?;
    fixture.node("ExtensionRuleFamily")?;
    fixture.link(key, CoreRelationshipTypeName::DescribedBy, "ExtensionRuleFamily")?;
    fixture.link("Contract", CoreRelationshipTypeName::ValidationBindings, key)?;
    let mut subject = fixture.staged_subject("subject")?;
    subject.with_property_value("Title", "present")?;
    let assessment = assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!assessment.observations.dispatched_rule_keys.contains(key));
    let report = assessment.report;
    assert_eq!(report.violation_count(), 1);
    assert_eq!(report.violations[0].kind, CommitValidationViolationKind::UnsupportedValidationRule);
    assert_eq!(report.violations[0].rule_key.as_deref(), Some(key));
    Ok(())
}

#[test]
fn split_saved_schema_and_staged_binding_definitions_are_compatible() -> Result<(), HolonError> {
    use crate::contexts::SubjectLevel;
    use holons_core::ReadableHolon;
    let fixture = readiness_fixture()?.saved_snapshot()?;
    // Earlier definitions are persisted; a later subject is staged independently.
    let mut subject = fixture.staged_subject("later-subject")?;
    subject.with_property_value("Title", "valid")?;
    assert!(validate_commit_candidates(&fixture.context, &[subject])?.is_accepted());

    let rule_key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let rule_update = fixture.stage_update_of(rule_key)?;
    let _family_update = fixture.stage_update_of("HolonValidationRule.HolonType")?;
    let _descriptor_update = fixture.stage_update_of("HolonType.TypeDescriptor")?;
    let context = ValueValidationContext::new(&fixture.context);
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
        &context
    )
    .unwrap());
    assert!(holons_core::same_definition(&binding.rule, &rule_update.clone().into())?);
    // Saved binding discovery preserves both the rule and its actual declaring descriptor.
    let contributions = holons_core::descriptors::effective_relationship_targets(
        &fixture.nodes["Contract"],
        CoreRelationshipTypeName::ValidationBindings,
    )
    .unwrap();
    assert_eq!(contributions.len(), 2);
    assert!(matches!(&contributions[0].member, HolonReference::Smart(_)));
    assert!(matches!(&contributions[0].declared_on, HolonReference::Smart(_)));
    assert_eq!(contributions[0].member.key()?.unwrap().to_string(), rule_key);
    Ok(())
}

#[test]
fn binding_family_and_constraint_type_follow_explicit_references() -> Result<(), HolonError> {
    use holons_core::{EffectiveRelationshipMember, HolonCollection, HolonCollectionApi};
    use std::sync::{Arc, RwLock};
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let rule_key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let wrong_family = fixture.nodes["PropertyValidationRule.HolonType"].clone();
    let update = fixture.stage_update_of_with(rule_key, |model| {
        let mut members = HolonCollection::new_transient();
        members.add_references(vec![wrong_family.clone()])?;
        model.relationships.as_mut().unwrap().insert(
            CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
            Arc::new(RwLock::new(members)),
        );
        Ok(())
    })?;
    let context = ValueValidationContext::new(&fixture.context);
    for (rule, compatible) in [(fixture.nodes[rule_key].clone(), true), (update.into(), false)] {
        let binding = ResolvedValidationBinding {
            rule,
            declaring_descriptor: holons_core::HolonDescriptor::from_holon(
                fixture.nodes["HolonType.TypeDescriptor"].clone(),
            ),
        };
        assert_eq!(
            compatible_binding_in_view(
                &binding,
                &fixture.nodes["Contract"],
                crate::contexts::SubjectLevel::Holon,
                &context
            )?,
            compatible
        );
        let constraint = ResolvedConstraint::resolve(EffectiveRelationshipMember {
            member: binding.rule,
            declared_on: fixture.nodes["HolonType.TypeDescriptor"].clone(),
        })?;
        assert_eq!(
            holons_core::same_definition(constraint.constraint_type.holon(), &wrong_family)?,
            !compatible
        );
    }
    Ok(())
}

#[path = "schema_assessment_tests.rs"]
mod schema_assessment;

/// Both public assessment modes must reject malformed ancestry before preparation or writes.
#[test]
fn staged_ancestry_invariant_rejects_reconstructed_updates_in_both_assessment_modes(
) -> Result<(), HolonError> {
    use holons_core::core_shared_objects::{holon::StagedState, WritableRelationship};
    #[derive(Clone, Copy)]
    enum AncestryCase {
        Missing,
        Duplicate,
        Mismatched,
        Touched,
    }
    use AncestryCase::*;
    for edge in [Missing, Duplicate, Mismatched, Touched] {
        for state in [
            StagedState::ForUpdate,
            StagedState::ForUpdateGraphOnly,
            StagedState::ForUpdateNewVersion,
        ] {
            let fixture = readiness_fixture()?.saved_snapshot()?;
            let predecessor = CoreRelationshipTypeName::Predecessor.as_relationship_name();
            let candidate = fixture.stage_update_snapshot(
                "Contract",
                |model| {
                    let mut relationships = holons_core::StagedRelationshipMap::new_empty();
                    let members = match edge {
                        Missing => vec![],
                        Duplicate => vec![fixture.nodes["Contract"].clone(); 2],
                        Mismatched => vec![fixture.nodes["TypeDescriptor"].clone()],
                        Touched => vec![fixture.nodes["Contract"].clone()],
                    };
                    relationships.add_related_holons_with_keys(
                        predecessor.clone(),
                        members.into_iter().map(|r| (r, None)).collect(),
                    )?;
                    model.relationships = Some(holons_core::RelationshipMap::from(relationships));
                    Ok(())
                },
                false,
            )?;
            if matches!(edge, Touched) {
                let mut handle = candidate.clone();
                handle.with_predecessor(Some(fixture.nodes["Contract"].clone()))?;
                assert_eq!(handle.staged_state()?, StagedState::ForUpdate);
                assert_eq!(handle.predecessor()?, Some(fixture.nodes["Contract"].clone()));
            }
            {
                let handle = candidate.get_holon_to_commit(&fixture.context)?;
                let mut holon = handle.write().unwrap();
                let Holon::Staged(staged) = &mut *holon else { unreachable!() };
                match state {
                    StagedState::ForUpdateGraphOnly => staged.note_relationship_mutation(false)?,
                    StagedState::ForUpdateNewVersion => staged.note_property_mutation()?,
                    _ => {}
                }
            }
            let before = candidate.get_holon_to_commit(&fixture.context)?.read().unwrap().clone();
            let reads = fixture.backend_relationship_reads();
            for error in [
                assess_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))
                    .unwrap_err(),
                validate_commit_candidates_with_profile(
                    &fixture.context,
                    std::slice::from_ref(&candidate),
                    |_| {},
                )
                .unwrap_err(),
            ] {
                let HolonError::InvalidState(message) = error else { panic!("{error:?}") };
                assert!(message.contains(&candidate.reference_id_string()));
                assert!(message.contains("Predecessor"));
                assert!(message.contains("observed"));
            }
            assert_eq!(fixture.backend_relationship_reads(), reads);
            assert_eq!(*candidate.get_holon_to_commit(&fixture.context)?.read().unwrap(), before);
        }
    }
    Ok(())
}

#[test]
fn generic_predecessor_edit_promotes_and_is_rejected_by_both_assessment_modes(
) -> Result<(), HolonError> {
    use holons_core::core_shared_objects::holon::StagedState;
    let mut fixture = readiness_fixture()?;
    fixture
        .node("Predecessor.Relationship")?
        .with_property_value("TypeName", "Predecessor")?
        .with_property_value("IsDefinitional", true)?
        .with_property_value("AllowsDuplicates", false)?;
    fixture.link(
        "Predecessor.Relationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType",
    )?;
    fixture.link(
        "Contract",
        CoreRelationshipTypeName::InstanceRelationships,
        "Predecessor.Relationship",
    )?;
    fixture.node("AncestrySubject")?;
    fixture.link("AncestrySubject", CoreRelationshipTypeName::DescribedBy, "Contract")?;
    let fixture = fixture.saved_snapshot()?;
    let mut candidate = fixture.stage_update_of("AncestrySubject")?;
    candidate.remove_related_holons(
        CoreRelationshipTypeName::Predecessor,
        vec![fixture.nodes["AncestrySubject"].clone()],
    )?;
    assert_eq!(candidate.staged_state()?, StagedState::ForUpdateNewVersion);
    assert!(matches!(
        assess_commit_candidates(&fixture.context, std::slice::from_ref(&candidate)),
        Err(HolonError::InvalidState(_))
    ));
    assert!(matches!(
        validate_commit_candidates_with_profile(&fixture.context, &[candidate], |_| {}),
        Err(HolonError::InvalidState(_))
    ));
    Ok(())
}

/// A saved contract's successor is a separate subject, never a substituted prerequisite.
#[test]
fn saved_book_keeps_title_v1_while_explicit_book_v2_depends_on_invalid_title_v2(
) -> Result<(), HolonError> {
    use holons_core::{HolonCollection, HolonCollectionApi};
    use std::sync::{Arc, RwLock};
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let title = fixture.stage_update_of_with("Title.PropertyType", |model| {
        use type_names::ToPropertyName;
        model.properties.insert(
            "IsValueRequired".to_property_name(),
            BaseValue::BooleanValue(MapBoolean(false)),
        );
        Ok(())
    })?;
    let book = fixture.stage_update_of_with("Contract", |model| {
        let mut members = HolonCollection::new_transient();
        members.add_references(vec![
            title.clone().into(),
            fixture.nodes["Key.PropertyType"].clone(),
        ])?;
        model.relationships.as_mut().unwrap().insert(
            CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
            Arc::new(RwLock::new(members)),
        );
        Ok(())
    })?;
    let mut saved_consumer = fixture.staged_subject("saved-book-consumer")?;
    saved_consumer.with_property_value("Title", "v1")?;
    // Author the reference before attachment so malformed supporting content is
    // diagnosed by assessment rather than attachment default preparation.
    let mut input = fixture.context.mutation().new_holon(Some("explicit-book-consumer".into()))?;
    input.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![book.clone().into()])?;
    let explicit_consumer = fixture.context.mutation().stage_new_holon(input)?;
    for reversed in [false, true] {
        let mut candidates =
            vec![saved_consumer.clone(), explicit_consumer.clone(), title.clone(), book.clone()];
        if reversed {
            candidates.reverse();
        }
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(saved_consumer.validation_state()?, ValidationState::Validated);
        assert!(saved_consumer.validation_findings()?.is_empty());
        assert_ne!(title.validation_state()?, ValidationState::Validated);
        assert!(explicit_consumer.validation_findings()?.iter().any(|finding| matches!(
            finding.kind,
            CommitValidationViolationKind::UnresolvedLocalDependency
        )));
        let package = holons_core::ContractContributions::resolve(&book.clone().into())?;
        assert!(package.properties.iter().any(|member| holons_core::same_definition(
            &member.member,
            &title.clone().into()
        )
        .unwrap()));
        assert!(holons_core::ContractContributions::resolve(&fixture.nodes["Contract"])?
            .properties
            .iter()
            .any(|member| holons_core::same_definition(
                &member.member,
                &fixture.nodes["Title.PropertyType"]
            )
            .unwrap()));
    }
    Ok(())
}

fn saved_instance_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture
        .node("Predecessor.Relationship")?
        .with_property_value("TypeName", "Predecessor")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Predecessor.Relationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link(
        "Predecessor.Relationship",
        CoreRelationshipTypeName::SourceType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "Predecessor.Relationship",
        CoreRelationshipTypeName::TargetType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "HolonType.TypeDescriptor",
        CoreRelationshipTypeName::InstanceRelationships,
        "Predecessor.Relationship",
    )?;
    let mut instance = fixture.staged_subject("SavedInstance")?;
    instance.with_property_value("Title", "original")?;
    fixture.nodes.insert("SavedInstance".into(), instance.into());
    fixture.saved_snapshot()
}

#[test]
fn version_producing_siblings_each_validate_and_receive_outcomes() -> Result<(), HolonError> {
    let fixture = saved_instance_fixture()?;
    let mut first = fixture.stage_update_of("SavedInstance")?;
    let mut second = fixture.stage_update_of("SavedInstance")?;
    first.with_property_value("Title", "first")?;
    second.with_property_value("Title", "second")?;
    assert_ne!(
        holons_core::DefinitionIdentity::for_reference(&first.clone().into(), &fixture.context)?,
        holons_core::DefinitionIdentity::for_reference(&second.clone().into(), &fixture.context)?
    );
    for candidates in [[first.clone(), second.clone()], [second.clone(), first.clone()]] {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(report.is_accepted(), "{report:?}");
        assert_eq!(first.validation_state()?, ValidationState::Validated);
        assert_eq!(second.validation_state()?, ValidationState::Validated);
    }
    Ok(())
}

#[test]
fn no_op_and_graph_only_siblings_are_both_assessed_without_poisoning_saved_definition(
) -> Result<(), HolonError> {
    use holons_core::core_shared_objects::WriteableHolonState;
    let fixture = saved_instance_fixture()?;
    let clean = fixture.stage_update_of("SavedInstance")?;
    let graph = fixture.stage_update_of("SavedInstance")?;
    // Assemble an unsupported relationship as a graph-only mutation. Its finding belongs
    // to this candidate; it does not invalidate the immutable saved definition.
    {
        let handle = graph.get_holon_to_commit(&fixture.context)?;
        let mut holon = handle.write().unwrap();
        let Holon::Staged(staged) = &mut *holon else { unreachable!() };
        staged.add_related_holons_with_keys(
            core_types::RelationshipName("Unlicensed".into()),
            vec![(fixture.nodes["Contract"].clone(), None)],
        )?;
        staged.note_relationship_mutation(false)?;
    }
    let mut consumer = fixture.staged_subject("consumer")?;
    consumer.with_property_value("Title", "valid")?;
    for candidates in [
        vec![clean.clone(), graph.clone(), consumer.clone()],
        vec![graph.clone(), consumer.clone(), clean.clone()],
    ] {
        clean.replace_validation_outcome(ValidationState::Invalid, Vec::new())?;
        graph.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(clean.validation_state()?, ValidationState::Validated);
        assert_eq!(graph.validation_state()?, ValidationState::Invalid);
        assert!(graph.validation_findings()?.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "UndeclaredRelationship")));
        assert_eq!(consumer.validation_state()?, ValidationState::Validated);
    }
    Ok(())
}

#[test]
fn shared_definition_product_findings_reach_both_no_op_and_graph_only_candidates(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    fixture.link(
        "Contract",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    fixture.link(
        "MetaHolonType.MetaTypeDescriptor",
        CoreRelationshipTypeName::ValidationBindings,
        CoreValidationRuleName::WellFormedEffectiveMemberDefinitions.as_str(),
    )?;
    let fixture = fixture.saved_snapshot()?;
    let first = fixture.stage_update_of("Contract")?;
    let second = fixture.stage_update_of("Contract")?;
    {
        let handle = second.get_holon_to_commit(&fixture.context)?;
        let mut holon = handle.write().unwrap();
        let Holon::Staged(staged) = &mut *holon else { unreachable!() };
        staged.note_relationship_mutation(false)?;
    }
    for candidates in [[first.clone(), second.clone()], [second.clone(), first.clone()]] {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        for candidate in &candidates {
            assert_eq!(candidate.validation_state()?, ValidationState::Invalid);
            assert!(candidate.validation_findings()?.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONTRACT-003")));
        }
    }
    Ok(())
}

#[test]
fn finding_paths_distinguish_candidates_with_the_same_display_prefix() -> Result<(), HolonError> {
    let fixture = Fixture::empty()?;
    let handles = ["12345678-0000-0000-0000-000000000001", "12345678-0000-0000-0000-000000000002"]
        .map(|id| {
            holons_core::StagedReference::from_temporary_id(
                fixture.context.context_handle(),
                &core_types::TemporaryId(id.parse().unwrap()),
            )
        });
    assert_eq!(handles[0].reference_id_string(), handles[1].reference_id_string());
    assert_ne!(
        crate::assessment_support::path(&handles[0].clone().into()),
        crate::assessment_support::path(&handles[1].clone().into())
    );
    Ok(())
}

#[test]
fn downstream_extends_saved_definition_stays_ready_when_graph_only_alias_fails(
) -> Result<(), HolonError> {
    use holons_core::core_shared_objects::WriteableHolonState;
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    fixture
        .node("Predecessor.AbstractRelationship")?
        .with_property_value("TypeName", "Predecessor")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Predecessor.AbstractRelationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link(
        "Predecessor.AbstractRelationship",
        CoreRelationshipTypeName::SourceType,
        "AbstractMetaType",
    )?;
    fixture.link(
        "Predecessor.AbstractRelationship",
        CoreRelationshipTypeName::TargetType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "AbstractMetaType",
        CoreRelationshipTypeName::InstanceRelationships,
        "Predecessor.AbstractRelationship",
    )?;
    let parent = abstract_subject(&fixture)?;
    fixture.nodes.insert("abstract-subject".into(), parent.into());
    let fixture = fixture.saved_snapshot()?;
    let clean = fixture.stage_update_of("abstract-subject")?;
    let graph = fixture.stage_update_of("abstract-subject")?;
    {
        let handle = graph.get_holon_to_commit(&fixture.context)?;
        let mut holon = handle.write().unwrap();
        let Holon::Staged(staged) = &mut *holon else { unreachable!() };
        staged.add_related_holons_with_keys(
            core_types::RelationshipName("Unlicensed".into()),
            vec![(fixture.nodes["HolonType.TypeDescriptor"].clone(), None)],
        )?;
        staged.note_relationship_mutation(false)?;
    }
    let mut input = fixture.context.mutation().new_holon(Some("child".into()))?;
    for (edge, target) in [
        (CoreRelationshipTypeName::Extends, "abstract-subject"),
        (CoreRelationshipTypeName::ComponentOf, "AbstractSchema"),
        (CoreRelationshipTypeName::DescribedBy, "AbstractMetaType"),
    ] {
        input.add_related_holons(edge, vec![fixture.nodes[target].clone()])?;
    }
    input
        .with_property_value("IsAbstractType", true)?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    let child = fixture.context.mutation().stage_new_holon(input)?;
    assert!(
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&child))?.is_accepted()
    );
    for candidates in [
        vec![graph.clone(), clean.clone(), child.clone()],
        vec![child.clone(), clean.clone(), graph.clone()],
    ] {
        let report = validate_commit_candidates(&fixture.context, &candidates)?;
        assert!(!report.is_accepted());
        assert_eq!(graph.validation_state()?, ValidationState::Invalid);
        assert!(graph.validation_findings()?.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "UndeclaredRelationship")));
        assert_eq!(clean.validation_state()?, ValidationState::Validated);
        assert_eq!(child.validation_state()?, ValidationState::Validated);
        assert!(child.validation_findings()?.is_empty());
    }
    Ok(())
}

#[test]
fn type_descriptor_requires_the_independent_meta_holon_type_role() -> Result<(), HolonError> {
    use holons_core::StructuralPrerequisites;
    let mut fixture = Fixture::new()?;
    let kinds = c2_kind_semantics(&mut fixture)?;
    let chosen = fixture.nodes["MetaTypeDescriptor.HolonType"].clone();
    let mut root = fixture.nodes["TypeDescriptor"].clone();
    root.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![chosen.clone()])?;
    let prerequisites = StructuralPrerequisites::assess(&root)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &kinds)?;
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescribingCategoryCompatibility,
        &products,
        &root,
    )?;
    assert_eq!(findings.len(), 1);
    assert!(matches!(&findings[0].kind,
        CommitValidationViolationKind::RuleViolation { code } if code == "DS-KIND-004"));
    root.remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![chosen])?;
    root.add_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone()],
    )?;
    let prerequisites = StructuralPrerequisites::assess(&root)?;
    let mut products = DescriptorRuleProducts::default();
    products.prepare_kind(&prerequisites, &kinds)?;
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::DescribingCategoryCompatibility,
        &products,
        &root
    )?
    .is_empty());
    Ok(())
}

#[test]
fn ordinary_unparented_holon_has_no_termination_finding() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    let ordinary: HolonReference =
        fixture.context.mutation().new_holon(Some("ordinary".into()))?.into();
    let prerequisites = holons_core::StructuralPrerequisites::assess(&ordinary)?;
    assert!(prerequisites.subject_lineage.valid_lineage().is_some());
    let mut products = DescriptorRuleProducts::default();
    products.prepare_structure(&prerequisites);
    assert!(dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::ExtendsLineageTerminatesAtTypeDescriptor,
        &products,
        &ordinary
    )?
    .is_empty());
    Ok(())
}

#[test]
fn every_builtin_rule_has_a_registration_and_only_cross_schema_dependencies_is_unimplemented() {
    for name in CoreValidationRuleName::all() {
        let registration = crate::registries::registration(name);
        assert_eq!(
            registration.implementation.is_none(),
            name == CoreValidationRuleName::CrossSchemaDependenciesDeclared,
            "{name:?}"
        );
    }
}

#[test]
fn unsupported_bindings_prepare_no_family_reads() -> Result<(), HolonError> {
    use holons_core::EffectiveRelationshipMember;
    for key in [
        "Extension.ValidationRule",
        CoreValidationRuleName::CrossSchemaDependenciesDeclared.as_str(),
    ] {
        for describers in [0, 1, 2] {
            let mut fixture = readiness_fixture()?;
            fixture.node(key)?;
            let families: Vec<_> =
                ["HolonValidationRule.HolonType", "PropertyValidationRule.HolonType"]
                    .into_iter()
                    .take(describers)
                    .map(|key| fixture.nodes[key].clone())
                    .collect();
            if !families.is_empty() {
                fixture
                    .nodes
                    .get_mut(key)
                    .unwrap()
                    .add_related_holons(CoreRelationshipTypeName::DescribedBy, families)?;
            }
            let fixture = fixture.saved_snapshot_with_read_errors(&[(
                "HolonValidationRule.HolonType",
                HolonError::InvalidState("unsupported family must not be read".into()),
            )])?;
            let contribution = EffectiveRelationshipMember {
                member: fixture.nodes[key].clone(),
                declared_on: fixture.nodes["Contract"].clone(),
            };
            let before = fixture.backend_relationship_reads();
            crate::subject_assessment::BindingRoleMemo::prepare(
                &fixture.context,
                [&contribution],
                std::iter::empty(),
            )?;
            assert_eq!(
                before,
                fixture.backend_relationship_reads(),
                "{key}: {describers} describers"
            );
            assert_eq!(fixture.backend_node_reads(), 1, "only the dispatch key's record is needed");
        }
    }
    Ok(())
}

#[test]
fn registered_binding_with_missing_or_ambiguous_describer_is_incompatible() -> Result<(), HolonError>
{
    for ambiguous in [false, true] {
        let mut fixture = readiness_fixture()?;
        let key = CoreValidationRuleName::NoUndescribedProperties.as_str();
        let original = fixture.nodes["HolonValidationRule.HolonType"].clone();
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original.clone()])?;
        if ambiguous {
            let other = fixture.nodes["PropertyValidationRule.HolonType"].clone();
            fixture
                .nodes
                .get_mut(key)
                .unwrap()
                .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original, other])?;
        }
        let mut subject = fixture.staged_subject("malformed-known-binding")?;
        subject.with_property_value("Title", "ready")?;
        let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
        assert!(!assessment.observations.dispatched_rule_keys.contains(key));
        assert_eq!(assessment.report.violation_count(), 1);
        assert!(
            matches!(&assessment.report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "IncompatibleValidationBinding")
        );
    }
    Ok(())
}

#[test]
fn wrong_declaring_role_and_native_kind_prevent_dispatch() -> Result<(), HolonError> {
    for wrong_native_kind in [false, true] {
        let mut fixture = readiness_fixture()?;
        let name = if wrong_native_kind {
            CoreValidationRuleName::BaseValueKindMatchesString
        } else {
            CoreValidationRuleName::NoUndescribedProperties
        };
        let key = name.as_str();
        if wrong_native_kind {
            fixture
                .nodes
                .get_mut("StringValueType.ValueType")
                .unwrap()
                .with_property_value("TypeName", "IntegerValueType")?;
        } else {
            let rule = fixture.nodes[key].clone();
            fixture
                .nodes
                .get_mut("HolonType.TypeDescriptor")
                .unwrap()
                .remove_related_holons(CoreRelationshipTypeName::ValidationBindings, vec![rule])?;
            fixture.link("TypeDescriptor", CoreRelationshipTypeName::ValidationBindings, key)?;
        }
        let mut subject = fixture.staged_subject("incompatible-placement")?;
        subject.with_property_value("Title", "ready")?;
        subject.remove_property_value("Key")?;
        let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
        assert!(!assessment.observations.dispatched_rule_keys.contains(key));
        assert!(assessment.report.violations.iter().any(|finding| finding.rule_key.as_deref() == Some(key)
            && matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "IncompatibleValidationBinding")));
    }
    Ok(())
}

#[test]
fn prerequisite_products_remain_reported_without_compatible_bindings() -> Result<(), HolonError> {
    use CoreValidationRuleName::*;
    let fixture = readiness_fixture()?;
    let subject = fixture.nodes["Contract"].clone();
    let mut products = DescriptorRuleProducts::default();
    let prerequisites = [
        (AtMostOneDirectParent, "DS-STRUCT-002"),
        (AcyclicExtendsLineage, "DS-STRUCT-003"),
        (ExtendsLineageTerminatesAtTypeDescriptor, "DS-STRUCT-004"),
        (UniqueTypeDescriptorRoot, "DS-STRUCT-005"),
        (DescribingCategoryCompatibility, "DS-KIND-004"),
        (DescriptorMetaTypeCorrespondence, "DS-KIND-005"),
    ];
    for (name, _) in prerequisites {
        products.violation(name, "prerequisite violation".into());
    }
    products.unresolved(DescribingCategoryCompatibility, "unresolved prerequisite".into());
    products.violation(LocalInstanceKindAnchorDesignation, "not a prerequisite".into());
    let mut collector = ValidationCollector::default();
    crate::subject_assessment::dispatch_descriptor(&[], &products, &subject, &mut collector)?;
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 7);
    for (name, expected_code) in prerequisites {
        assert!(report.violations.iter().any(|finding| finding.rule_key.as_deref() == Some(name.as_str())
            && matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == expected_code)));
    }
    assert!(report
        .violations
        .iter()
        .any(|finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    assert!(report.violations.iter().all(|finding| finding.descriptor_identity.is_none()));
    Ok(())
}

#[test]
fn direct_descriptor_dispatch_preserves_binding_provenance_and_diagnostic_kinds(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let name = CoreValidationRuleName::AtMostOneDirectParent;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::ValidationBindings,
        name.as_str(),
    )?;
    let governor = &fixture.nodes["MetaTypeDescriptor.HolonType"];
    let subject = &fixture.nodes["Contract"];
    let workset = crate::schema_view::SchemaWorkset { schemas: Vec::new() };
    let packages = crate::descriptor_package::DescriptorPackages::construct(
        &fixture.context,
        &[governor.clone()],
        &[],
        &workset,
    )?;
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(governor)?;
    let roles = crate::subject_assessment::BindingRoleMemo::prepare(
        &fixture.context,
        packages.binding_contributions(),
        diagnosis.valid_lineage(),
    )?;
    let mut collector = ValidationCollector::default();
    let bindings = crate::subject_assessment::prepare_bindings(
        governor,
        crate::contexts::SubjectLevel::Holon,
        &ValueValidationContext::new(&fixture.context),
        &roles,
        &crate::assessment_support::path(subject),
        &mut collector,
        &packages,
    )?;
    let mut products = DescriptorRuleProducts::default();
    products.violation(name, "multiple parents".into());
    products.unresolved(name, "unresolved input".into());
    crate::subject_assessment::dispatch_descriptor(&bindings, &products, subject, &mut collector)?;
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 2, "fallback must not duplicate dispatched products");
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-STRUCT-002")
    );
    assert_eq!(report.violations[1].kind, CommitValidationViolationKind::UnresolvedLocalDependency);
    assert!(report
        .violations
        .iter()
        .all(|finding| finding.rule_key.as_deref() == Some(name.as_str())
            && finding.descriptor_identity == Some(governor.reference_id_string())));
    Ok(())
}

#[test]
fn equal_dispatch_keys_keep_exact_family_versions_in_the_role_memo() -> Result<(), HolonError> {
    use holons_core::{EffectiveRelationshipMember, HolonCollection, HolonCollectionApi};
    use std::sync::{Arc, RwLock};
    let mut fixture = readiness_fixture()?;
    let name = CoreValidationRuleName::NoUndescribedProperties;
    let key = name.as_str();
    fixture.node("RuleFamilySubtype")?;
    fixture.link(
        "RuleFamilySubtype",
        CoreRelationshipTypeName::Extends,
        "HolonValidationRule.HolonType",
    )?;
    let original = fixture.nodes["HolonValidationRule.HolonType"].clone();
    fixture
        .nodes
        .get_mut(key)
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original])?;
    fixture.link(key, CoreRelationshipTypeName::DescribedBy, "RuleFamilySubtype")?;
    let fixture = fixture.saved_snapshot()?;
    let wrong_parent = fixture.nodes["PropertyValidationRule.HolonType"].clone();
    let family_successor: HolonReference = fixture
        .stage_update_of_with("RuleFamilySubtype", |model| {
            let mut members = HolonCollection::new_transient();
            members.add_references(vec![wrong_parent])?;
            model.relationships.as_mut().unwrap().insert(
                CoreRelationshipTypeName::Extends.as_relationship_name(),
                Arc::new(RwLock::new(members)),
            );
            Ok(())
        })?
        .into();
    let mut supported_successor = fixture.stage_update_of(key)?;
    supported_successor.with_property_value("Description", "distinct supported definition")?;
    let unsupported_successor: HolonReference = fixture
        .stage_update_of_with(key, |model| {
            let mut members = HolonCollection::new_transient();
            members.add_references(vec![family_successor.clone()])?;
            model.relationships.as_mut().unwrap().insert(
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                Arc::new(RwLock::new(members)),
            );
            Ok(())
        })?
        .into();
    let supported_successor: HolonReference = supported_successor.into();
    assert!(!holons_core::same_definition(&fixture.nodes[key], &supported_successor)?);
    let declaring = &fixture.nodes["HolonType.TypeDescriptor"];
    let contributions: Vec<_> =
        [fixture.nodes[key].clone(), supported_successor, unsupported_successor]
            .into_iter()
            .map(|member| EffectiveRelationshipMember { member, declared_on: declaring.clone() })
            .collect();
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(&fixture.nodes["Contract"])?;
    let roles = crate::subject_assessment::BindingRoleMemo::prepare(
        &fixture.context,
        &contributions,
        diagnosis.valid_lineage(),
    )?;
    let reads_after_preparation =
        (fixture.backend_node_reads(), fixture.backend_relationship_reads());
    for (contribution, expected) in contributions.into_iter().zip([true, true, false]) {
        let binding = ResolvedValidationBinding::from(contribution);
        assert_eq!(binding.rule.key()?.as_ref().map(|key| key.0.as_str()), Some(key));
        let holons_core::DescribingTypeResolution::Unique(family) =
            holons_core::resolve_describing_type(&binding.rule)?
        else {
            panic!("unique test family");
        };
        assert_eq!(
            crate::subject_assessment::compatible_binding(
                &binding,
                &family,
                crate::registries::registration(name),
                &fixture.nodes["Contract"],
                crate::contexts::SubjectLevel::Holon,
                &ValueValidationContext::new(&fixture.context),
                &roles
            )?,
            expected
        );
    }
    assert_eq!(
        reads_after_preparation,
        (fixture.backend_node_reads(), fixture.backend_relationship_reads())
    );
    Ok(())
}

#[test]
fn required_binding_role_read_errors_are_preserved_without_retrying_backend_reads(
) -> Result<(), HolonError> {
    use holons_core::EffectiveRelationshipMember;
    let error = HolonError::EmptyField("required rule-family backend failure".into());
    let name = CoreValidationRuleName::NoUndescribedProperties;
    let mut fixture = readiness_fixture()?;
    fixture.node("FamilySubtype")?;
    fixture.link(
        "FamilySubtype",
        CoreRelationshipTypeName::Extends,
        "HolonValidationRule.HolonType",
    )?;
    let original = fixture.nodes["HolonValidationRule.HolonType"].clone();
    fixture
        .nodes
        .get_mut(name.as_str())
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original])?;
    fixture.link(name.as_str(), CoreRelationshipTypeName::DescribedBy, "FamilySubtype")?;
    let fixture = fixture
        .saved_snapshot_with_read_errors(&[("HolonValidationRule.HolonType", error.clone())])?;
    let contribution = EffectiveRelationshipMember {
        member: fixture.nodes[name.as_str()].clone(),
        declared_on: fixture.nodes["HolonType.TypeDescriptor"].clone(),
    };
    let roles = crate::subject_assessment::BindingRoleMemo::prepare(
        &fixture.context,
        [&contribution],
        std::iter::empty(),
    )?;
    let reads_after_preparation =
        (fixture.backend_node_reads(), fixture.backend_relationship_reads());
    let binding = ResolvedValidationBinding::from(contribution);
    assert_eq!(
        crate::subject_assessment::compatible_binding(
            &binding,
            &fixture.nodes["FamilySubtype"],
            crate::registries::registration(name),
            &fixture.nodes["Contract"],
            crate::contexts::SubjectLevel::Holon,
            &ValueValidationContext::new(&fixture.context),
            &roles
        ),
        Err(error)
    );
    assert_eq!(
        reads_after_preparation,
        (fixture.backend_node_reads(), fixture.backend_relationship_reads())
    );
    Ok(())
}

#[test]
fn registrations_match_independent_core_placement_expectations() {
    use crate::contexts::SubjectLevel::{Holon, Property, Value};
    use std::collections::BTreeMap;
    // Keep these expectations independent of registration-driven fixture setup.
    let expected = [
        (
            "RequiredPropertyPresence.ValidationRule",
            ("PropertyValidationRule.HolonType", "PropertyType.TypeDescriptor", Property),
        ),
        (
            "NoUndescribedProperties.ValidationRule",
            ("HolonValidationRule.HolonType", "HolonType.TypeDescriptor", Holon),
        ),
        (
            "KeyUniqueness.ValidationRule",
            ("HolonValidationRule.HolonType", "HolonType.TypeDescriptor", Holon),
        ),
        (
            "BaseValueKindMatchesString.ValidationRule",
            ("StringValidationRule.HolonType", "StringValueType.ValueType", Value),
        ),
        (
            "BaseValueKindMatchesInteger.ValidationRule",
            ("IntegerValidationRule.HolonType", "IntegerValueType.ValueType", Value),
        ),
        (
            "BaseValueKindMatchesBoolean.ValidationRule",
            ("BooleanValidationRule.HolonType", "BooleanValueType.ValueType", Value),
        ),
        (
            "BaseValueKindMatchesBytes.ValidationRule",
            ("BytesValidationRule.HolonType", "BytesValueType.ValueType", Value),
        ),
        (
            "BaseValueKindMatchesEnum.ValidationRule",
            ("EnumValueValidationRule.HolonType", "EnumValueType.ValueType", Value),
        ),
        (
            "AtMostOneDirectParent.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "AcyclicExtendsLineage.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "ExtendsLineageTerminatesAtTypeDescriptor.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "UniqueTypeDescriptorRoot.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "LocalInstanceKindAnchorDesignation.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "InstanceKindAnchorsAreAbstract.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "TypeDescriptorRootKindException.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "DescribingCategoryCompatibility.ValidationRule",
            ("HolonValidationRule.HolonType", "HolonType.TypeDescriptor", Holon),
        ),
        (
            "DescriptorMetaTypeCorrespondence.ValidationRule",
            ("HolonValidationRule.HolonType", "HolonType.TypeDescriptor", Holon),
        ),
        (
            "NoInheritedMemberRedeclaration.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "UniqueSemanticMemberNames.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "WellFormedEffectiveMemberDefinitions.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "ContractMemberKindCompatibility.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "InheritedValueConstraintNonRelaxation.ValidationRule",
            ("HolonValidationRule.HolonType", "MetaTypeDescriptor.HolonType", Holon),
        ),
        (
            "SchemaDependenciesAcyclic.ValidationRule",
            ("HolonValidationRule.HolonType", "Schema.HolonType", Holon),
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let actual = CoreValidationRuleName::all()
        .filter_map(|name| {
            let registration = crate::registries::registration(name);
            registration.implementation.map(|_| {
                (
                    name.as_str(),
                    (registration.family.key(), registration.declaring.key(), registration.level),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}

#[test]
fn holon_rule_with_only_the_wrong_family_is_rejected_before_dispatch() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let original_family = fixture.nodes["HolonValidationRule.HolonType"].clone();
    fixture
        .nodes
        .get_mut(key)
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::DescribedBy, vec![original_family])?;
    fixture.link(key, CoreRelationshipTypeName::DescribedBy, "PropertyValidationRule.HolonType")?;
    let mut subject = fixture.staged_subject("wrong-family")?;
    subject.with_property_value("Title", "present")?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
    assert!(!assessment.observations.dispatched_rule_keys.contains(key));
    assert_eq!(assessment.report.violation_count(), 1);
    let finding = &assessment.report.violations[0];
    assert_eq!(finding.rule_key.as_deref(), Some(key));
    assert_eq!(
        finding.descriptor_identity,
        Some(fixture.nodes["HolonType.TypeDescriptor"].reference_id_string())
    );
    assert!(
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "IncompatibleValidationBinding")
    );
    Ok(())
}

#[test]
fn binding_compatibility_rejects_only_a_changed_subject_level() -> Result<(), HolonError> {
    use crate::contexts::SubjectLevel;
    let fixture = readiness_fixture()?;
    let key = CoreValidationRuleName::NoUndescribedProperties.as_str();
    let binding = ResolvedValidationBinding {
        rule: fixture.nodes[key].clone(),
        declaring_descriptor: holons_core::HolonDescriptor::from_holon(
            fixture.nodes["HolonType.TypeDescriptor"].clone(),
        ),
    };
    let context = ValueValidationContext::new(&fixture.context);
    for level in [SubjectLevel::Holon, SubjectLevel::Property, SubjectLevel::Value] {
        assert_eq!(
            compatible_binding_in_view(&binding, &fixture.nodes["Contract"], level, &context)?,
            level == SubjectLevel::Holon
        );
    }
    Ok(())
}

#[test]
fn unknown_binding_preparation_skips_an_unreadable_describer() -> Result<(), HolonError> {
    for describers in [0, 1, 2] {
        let mut fixture = readiness_fixture()?;
        let key = "Unknown.ValidationRule";
        fixture.node(key)?;
        let families: Vec<_> =
            ["HolonValidationRule.HolonType", "PropertyValidationRule.HolonType"]
                .into_iter()
                .take(describers)
                .map(|key| fixture.nodes[key].clone())
                .collect();
        if !families.is_empty() {
            fixture
                .nodes
                .get_mut(key)
                .unwrap()
                .add_related_holons(CoreRelationshipTypeName::DescribedBy, families)?;
        }
        fixture.link("Contract", CoreRelationshipTypeName::ValidationBindings, key)?;
        let fixture = fixture.saved_snapshot()?;
        fixture.control_saved_relationship_reads(
            key,
            CoreRelationshipTypeName::DescribedBy,
            None,
        )?;
        let governor = &fixture.nodes["Contract"];
        let workset = crate::schema_view::SchemaWorkset { schemas: Vec::new() };
        let packages = crate::descriptor_package::DescriptorPackages::construct(
            &fixture.context,
            &[governor.clone()],
            &[],
            &workset,
        )?;
        let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(governor)?;
        let roles = crate::subject_assessment::BindingRoleMemo::prepare(
            &fixture.context,
            packages.binding_contributions(),
            diagnosis.valid_lineage(),
        )?;
        // Construction has its own schema traversal; this trap tests the dispatch ordering.
        let error = HolonError::InvalidState(
            "unknown rule describer must not be read during assessment".into(),
        );
        fixture.control_saved_relationship_reads(
            key,
            CoreRelationshipTypeName::DescribedBy,
            Some(error.clone()),
        )?;
        assert_eq!(
            fixture.nodes[key].related_holons(CoreRelationshipTypeName::DescribedBy).err(),
            Some(error)
        );
        let mut collector = ValidationCollector::default();
        let bindings = crate::subject_assessment::prepare_bindings(
            governor,
            crate::contexts::SubjectLevel::Holon,
            &ValueValidationContext::new(&fixture.context),
            &roles,
            &crate::assessment_support::path(governor),
            &mut collector,
            &packages,
        )?;
        assert_eq!(bindings.len(), 2, "the two supported inherited bindings remain available");
        assert!(collector.observations().discovered_rule_keys.contains(key));
        let report = collector.into_report();
        assert_eq!(report.violation_count(), 1);
        let finding = report
            .violations
            .iter()
            .find(|finding| finding.rule_key.as_deref() == Some(key))
            .expect("unsupported binding finding");
        assert_eq!(finding.kind, CommitValidationViolationKind::UnsupportedValidationRule);
        assert_eq!(
            finding.descriptor_identity,
            Some(fixture.nodes["Contract"].reference_id_string())
        );
    }
    Ok(())
}

#[test]
fn universal_membership_uses_identity_even_when_category_members_have_the_same_name(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    fixture
        .node("UniversalTitle")?
        .with_property_value("TypeName", "Title")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "UniversalTitle",
        CoreRelationshipTypeName::Extends,
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "UniversalTitle",
        CoreRelationshipTypeName::ValueType,
        "StringValueType.ValueType",
    )?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::InstanceProperties,
        "UniversalTitle",
    )?;
    let subject = abstract_subject(&fixture)?;
    let contract = holons_core::UniversalDescriptorContract::from_meta_type(
        &fixture.context,
        &fixture.nodes["MetaTypeDescriptor.HolonType"],
    )?;
    assert!(!contract.enforce_minimum(
        &subject.clone().into(),
        true,
        &fixture.nodes["Title.PropertyType"]
    )?);
    assert!(contract.enforce_minimum(&subject.into(), true, &fixture.nodes["UniversalTitle"])?);
    Ok(())
}

#[test]
fn ordinary_holon_with_abstractness_still_requires_category_properties() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let input = fixture.context.mutation().new_holon(Some("ordinary-abstract".into()))?;
    let mut subject = fixture.context.mutation().stage_new_holon(input)?;
    subject.with_descriptor(fixture.nodes["AbstractMetaType"].clone())?;
    subject
        .with_property_value("IsAbstractType", true)?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    let report = assess_commit_candidates(&fixture.context, &[subject])?.report;
    assert!(report.violations.iter().any(|finding| {
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
            && matches!(&finding.subject, ValidationSubjectPath::Property { name, .. } if name == "Title")
    }), "{report:?}");
    Ok(())
}

#[test]
fn abstract_descriptor_without_meta_type_role_enforces_missing_category_members(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let meta_parent = fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone();
    fixture
        .nodes
        .get_mut("AbstractMetaType")
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::Extends, vec![meta_parent])?;
    fixture.link(
        "AbstractMetaType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    let subject = abstract_subject(&fixture)?;
    let report = assess_commit_candidates(&fixture.context, &[subject])?.report;
    for expected in ["DS-KIND-005", "DS-PROP-001"] {
        assert!(report.violations.iter().any(|finding| {
            matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == expected)
        }), "missing {expected}: {report:?}");
    }
    Ok(())
}

#[test]
fn abstract_descriptor_universal_contract_needs_no_backend_reads_after_construction(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let fixture = fixture.saved_snapshot()?;
    let candidates = vec![abstract_subject(&fixture)?];
    let constructed = crate::commit_assessment::construct_assessment(
        &fixture.context,
        &candidates,
        ValidationCollector::default(),
    )?;
    let relationship_reads = fixture.backend_relationship_reads();
    let node_reads = fixture.backend_node_reads();
    let report = constructed.assess(|_| {})?.0.into_report();
    assert!(report.is_accepted(), "{report:?}");
    assert_eq!(relationship_reads, fixture.backend_relationship_reads());
    assert_eq!(node_reads, fixture.backend_node_reads());
    Ok(())
}

#[test]
fn encountered_required_definition_read_error_preserves_prior_outcomes() -> Result<(), HolonError> {
    let error = HolonError::InvalidState("unreadable governing definition".into());
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let input = fixture.context.mutation().new_holon(Some("subject".into()))?;
    let mut candidate = fixture.context.mutation().stage_new_holon(input)?;
    candidate.add_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![fixture.nodes["Contract"].clone()],
    )?;
    candidate.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    fixture.control_saved_relationship_reads(
        "Contract",
        CoreRelationshipTypeName::Extends,
        Some(error.clone()),
    )?;
    assert_eq!(
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate)).err(),
        Some(error)
    );
    assert_eq!(candidate.validation_state()?, ValidationState::Validated);
    assert!(candidate.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn mixed_meta_type_definitions_keep_separate_universal_contracts() -> Result<(), HolonError> {
    use holons_core::core_shared_objects::WritableRelationship;

    let mut fixture = readiness_fixture()?;
    declare_abstract_property(&mut fixture)?;
    let mut fixture = fixture.saved_snapshot()?;
    // The saved predecessor remains in the first governing lineage. Its successor
    // inherits Title through an exact saved category reference. Both meta-type versions
    // occur in the second governing lineage; the nearest role member must win.
    let successor = fixture.stage_update_of_with("MetaTypeDescriptor.HolonType", |model| {
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        relationships.map.remove(&CoreRelationshipTypeName::Extends.as_relationship_name());
        relationships.add_related_holons_with_keys(
            CoreRelationshipTypeName::Extends.as_relationship_name(),
            vec![(fixture.nodes["AbstractMetaType"].clone(), None)],
        )?;
        model.relationships = Some(holons_core::RelationshipMap::from(relationships));
        Ok(())
    })?;
    fixture.nodes.insert("MetaTypeV2".into(), successor.into());
    fixture.node("AbstractMetaV2")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("AbstractMetaV2", CoreRelationshipTypeName::Extends, "MetaTypeV2")?;
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(&fixture.nodes["AbstractMetaV2"])?;
    let lineage =
        diagnosis.valid_lineage().expect("both exact meta-type versions form a valid lineage");
    let selected = lineage
        .member_with_role(type_names::CoreDefinitionRole::MetaTypeDescriptor)
        .expect("governing lineage contains the meta-type role");
    assert!(lineage.contains(&fixture.nodes["MetaTypeDescriptor.HolonType"])?);
    assert!(holons_core::same_definition(selected, &fixture.nodes["MetaTypeV2"])?);
    assert!(!holons_core::same_definition(
        selected,
        &fixture.nodes["MetaTypeDescriptor.HolonType"]
    )?);
    let first = abstract_subject(&fixture)?;
    let mut second = abstract_subject(&fixture)?;
    second.with_property_value("Key", "second-abstract-subject")?;
    second.remove_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![fixture.nodes["AbstractMetaType"].clone()],
    )?;
    second.add_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![fixture.nodes["AbstractMetaV2"].clone()],
    )?;
    let first_identity = first.full_reference_id_string();
    let second_identity = second.full_reference_id_string();
    for candidates in [[first.clone(), second.clone()], [second.clone(), first.clone()]] {
        let report = assess_commit_candidates(&fixture.context, &candidates)?.report;
        let title_findings: Vec<_> = report.violations.iter().filter(|finding| {
            matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")
                && matches!(&finding.subject, ValidationSubjectPath::Property { name, .. } if name == "Title")
        }).collect();
        assert_eq!(title_findings.len(), 1, "{report:?}");
        assert!(matches!(&title_findings[0].subject,
            ValidationSubjectPath::Property { holon_identity, .. } if holon_identity == &second_identity));
        assert!(!report.violations.iter().any(|finding| {
            matches!(&finding.subject, ValidationSubjectPath::Property { holon_identity, .. } if holon_identity == &first_identity)
        }), "{report:?}");
    }
    Ok(())
}

#[test]
fn encountered_missing_lineage_root_reports_its_prerequisite_finding() -> Result<(), HolonError> {
    let mut fixture = Fixture::empty()?;
    fixture.node("unrelated-terminal")?;
    let mut input = fixture.context.mutation().new_holon(Some("rootless-subject".into()))?;
    input.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![fixture.nodes["unrelated-terminal"].clone()],
    )?;
    let candidate = fixture.context.mutation().stage_new_holon(input)?;
    let report = assess_commit_candidates(&fixture.context, &[candidate])?.report;
    assert!(report.violations.iter().any(|finding| {
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-STRUCT-004")
    }), "{report:?}");
    Ok(())
}

#[test]
fn saved_assessment_uses_bound_definitions_without_keyed_anchor_lookup() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let mut subject = fixture.staged_subject("bound-definition-consumer")?;
    subject.with_property_value("Title", "complete")?;
    let reads_before = fixture.saved_key_lookup_reads();
    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(report.is_accepted(), "{report:#?}");
    assert_eq!(fixture.saved_key_lookup_reads(), reads_before);

    // Removing anchor discovery must retain findings for encountered missing inputs.
    subject.remove_property_value("Title")?;
    let report = validate_commit_candidates(&fixture.context, &[subject])?;
    assert!(
        report.violations.iter().any(|finding| matches!(
            &finding.kind,
            CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001"
        )),
        "{report:#?}"
    );
    assert_eq!(fixture.saved_key_lookup_reads(), reads_before);

    // Positive control after assessment proves the fixture counter observes keyed lookup.
    fixture.context.lookup().get_saved_holon_by_key(&"Contract".into())?;
    assert_eq!(fixture.saved_key_lookup_reads(), reads_before + 1);
    Ok(())
}
