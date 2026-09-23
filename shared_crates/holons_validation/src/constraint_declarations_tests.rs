use super::*;
use holons_core::{
    descriptors::{same_definition, ConstraintContributions},
    CurrentDescriptorReader, DescriptorReader, ExtendsLineageDiagnosis, ReadableHolon,
};

const BOUNDED: [&str; 4] = [
    "StringLengthConstraint.ConstraintType",
    "BytesLengthConstraint.ConstraintType",
    "NumericRangeConstraint.ConstraintType",
    "ItemCountConstraint.ConstraintType",
];
const CARDINALITY: &str = "CardinalityConstraint.ConstraintType";
const UNIQUE: &str = "UniqueItemsConstraint.ConstraintType";

fn roots(fixture: &Fixture) -> ConstraintDeclarationRoots {
    ConstraintDeclarationRoots {
        type_descriptor: fixture.nodes["TypeDescriptor"].clone(),
        constraint_type: fixture.nodes["ConstraintType.HolonType"].clone(),
        bounded: BOUNDED.map(|key| fixture.nodes[key].clone()),
        cardinality: fixture.nodes[CARDINALITY].clone(),
        unique_items: fixture.nodes[UNIQUE].clone(),
    }
}

fn fixture() -> Result<Fixture, HolonError> {
    let mut fixture = Fixture::new()?;
    fixture.node("ConstraintType.HolonType")?;
    fixture.link(
        "ConstraintType.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    for key in BOUNDED.into_iter().chain([CARDINALITY, UNIQUE]) {
        fixture.node(key)?;
        fixture.link(key, CoreRelationshipTypeName::Extends, "ConstraintType.HolonType")?;
        fixture.link(
            key,
            CoreRelationshipTypeName::ApplicableToDescriptorTypes,
            "StringValueType.ValueType",
        )?;
    }
    fixture.node("Schema.HolonType")?;
    declare_relationship(&mut fixture, "ConstraintType.HolonType", "RuleOf", "Schema.HolonType")?;
    // Fixture metadata belongs to its explicit describing contract as well.
    for name in [
        "Key",
        "TypeName",
        "IsAbstractType",
        "IsValueRequired",
        "ConstraintName",
        "Minimum",
        "Maximum",
        "MinimumIsInclusive",
        "MaximumIsInclusive",
    ] {
        let key = format!("{name}.ConfigurationProperty");
        fixture.node(&key)?;
        fixture.nodes.get_mut(&key).unwrap().with_property_value("TypeName", name)?;
        if ["Minimum", "Maximum", "MinimumIsInclusive", "MaximumIsInclusive"].contains(&name) {
            for family in BOUNDED {
                fixture.link(family, CoreRelationshipTypeName::InstanceProperties, &key)?;
            }
            if ["Minimum", "Maximum"].contains(&name) {
                fixture.link(CARDINALITY, CoreRelationshipTypeName::InstanceProperties, &key)?;
            }
        } else {
            fixture.link(
                "ConstraintType.HolonType",
                CoreRelationshipTypeName::InstanceProperties,
                &key,
            )?;
        }
    }
    Ok(fixture)
}

/// Complete the relationship policy read by governed staged mutation.
fn declare_relationship(
    fixture: &mut Fixture,
    owner: &str,
    name: &str,
    target: &str,
) -> Result<(), HolonError> {
    let key = format!("{name}.ConstraintFixtureRelationship");
    let mut relationship = fixture.node(&key)?;
    relationship
        .with_property_value("TypeName", name)?
        .with_property_value("AllowsDuplicates", false)?
        .with_property_value("IsOrdered", false)?
        .with_property_value("IsDefinitional", true)?;
    fixture.link(&key, CoreRelationshipTypeName::Extends, "DeclaredRelationshipType")?;
    fixture.link(&key, CoreRelationshipTypeName::SourceType, owner)?;
    fixture.link(&key, CoreRelationshipTypeName::TargetType, target)?;
    fixture.link(owner, CoreRelationshipTypeName::InstanceRelationships, &key)
}

fn configured(
    fixture: &mut Fixture,
    key: &str,
    family: &str,
) -> Result<HolonReference, HolonError> {
    let constraint = fixture.node(key)?;
    fixture.link(key, CoreRelationshipTypeName::DescribedBy, family)?;
    Ok(constraint)
}

#[test]
fn bounded_configuration_boundaries_are_declarations_without_evaluators() -> Result<(), HolonError>
{
    for family in BOUNDED {
        for (minimum, maximum, lower, upper, accepted) in [
            (None, None, None, None, false),
            (Some(0), None, Some(true), None, true),
            (None, Some(8), None, Some(false), true),
            (Some(2), Some(8), Some(false), Some(false), true),
            (Some(2), Some(2), Some(true), Some(true), true),
            (Some(2), Some(2), Some(false), Some(true), false),
            (Some(2), Some(2), Some(true), Some(false), false),
            (Some(8), Some(2), Some(true), Some(true), false),
            (Some(-1), None, Some(true), None, false),
            (None, Some(-1), None, Some(true), false),
            (Some(0), None, None, None, false),
            (None, Some(8), None, None, false),
            (None, Some(8), Some(true), Some(true), false),
            (Some(0), None, Some(true), Some(true), false),
        ] {
            let mut fixture = fixture()?;
            let mut constraint = configured(&mut fixture, "Configured", family)?;
            for (name, value) in [("Minimum", minimum), ("Maximum", maximum)] {
                if let Some(value) = value {
                    constraint.with_property_value(name, value as i64)?;
                }
            }
            for (name, value) in [("MinimumIsInclusive", lower), ("MaximumIsInclusive", upper)] {
                if let Some(value) = value {
                    constraint.with_property_value(name, value)?;
                }
            }
            let roots = roots(&fixture);
            let mut assessment =
                ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
            let mut collector = ValidationCollector::default();
            assert_eq!(
                assessment.assess_constraint(&constraint, &mut collector)?,
                accepted,
                "{family}: {minimum:?} {maximum:?} {lower:?} {upper:?}"
            );
            assert_eq!(collector.observations().effective_constraint_count, 0);
            assert_eq!(collector.observations().constraint_declaration_count, 1);
            let report = collector.into_report();
            assert_eq!(report.is_accepted(), accepted);
            assert!(report.violations.iter().all(|finding| matches!(&finding.kind,
                CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONSTRAINT-003")));
        }
    }
    Ok(())
}

#[test]
fn cardinality_and_presence_only_uniqueness_have_distinct_configuration_contracts(
) -> Result<(), HolonError> {
    for (family, fields, accepted) in [
        (CARDINALITY, vec![], false),
        (CARDINALITY, vec![("Minimum", BaseValue::IntegerValue(MapInteger(0)))], true),
        (
            CARDINALITY,
            vec![
                ("Minimum", BaseValue::IntegerValue(MapInteger(2))),
                ("Maximum", BaseValue::IntegerValue(MapInteger(2))),
            ],
            true,
        ),
        (CARDINALITY, vec![("Minimum", BaseValue::IntegerValue(MapInteger(-1)))], false),
        (
            CARDINALITY,
            vec![
                ("Minimum", BaseValue::IntegerValue(MapInteger(2))),
                ("Maximum", BaseValue::IntegerValue(MapInteger(1))),
            ],
            false,
        ),
        (
            CARDINALITY,
            vec![
                ("Minimum", BaseValue::IntegerValue(MapInteger(0))),
                ("MinimumIsInclusive", BaseValue::BooleanValue(MapBoolean(true))),
            ],
            false,
        ),
        (UNIQUE, vec![], true),
        (UNIQUE, vec![("Enabled", BaseValue::BooleanValue(MapBoolean(false)))], false),
        (UNIQUE, vec![("Enabled", BaseValue::BooleanValue(MapBoolean(true)))], false),
        (UNIQUE, vec![("Minimum", BaseValue::IntegerValue(MapInteger(0)))], false),
        (
            BOUNDED[0],
            vec![
                ("Minimum", BaseValue::StringValue(MapString("0".into()))),
                ("MinimumIsInclusive", BaseValue::BooleanValue(MapBoolean(true))),
            ],
            false,
        ),
        (
            BOUNDED[0],
            vec![
                ("Minimum", BaseValue::IntegerValue(MapInteger(0))),
                ("MinimumIsInclusive", BaseValue::StringValue(MapString("true".into()))),
            ],
            false,
        ),
    ] {
        let mut fixture = fixture()?;
        let mut constraint = configured(&mut fixture, "Configured", family)?;
        for (name, value) in fields {
            constraint.with_property_value(name, value)?;
        }
        let roots = roots(&fixture);
        let mut collector = ValidationCollector::default();
        let mut assessment =
            ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
        assert_eq!(
            assessment.assess_constraint(&constraint, &mut collector)?,
            accepted,
            "{family}"
        );
        assert_eq!(collector.into_report().is_accepted(), accepted);
    }
    Ok(())
}

#[test]
fn broader_local_constraints_preserve_inherited_obligations_and_check_reusable_rules_once(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    let mut inherited = configured(&mut fixture, "Inherited", BOUNDED[0])?;
    inherited
        .with_property_value("Minimum", 3_i64)?
        .with_property_value("Maximum", 80_i64)?
        .with_property_value("MinimumIsInclusive", true)?
        .with_property_value("MaximumIsInclusive", true)?;
    let mut broader = configured(&mut fixture, "Broader", BOUNDED[0])?;
    broader
        .with_property_value("Minimum", 1_i64)?
        .with_property_value("Maximum", 100_i64)?
        .with_property_value("MinimumIsInclusive", true)?
        .with_property_value("MaximumIsInclusive", true)?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "Inherited",
    )?;
    fixture.node("Child")?;
    fixture.link("Child", CoreRelationshipTypeName::Extends, "StringValueType.ValueType")?;
    fixture.link("Child", CoreRelationshipTypeName::Constraints, "Broader")?;
    let roots = roots(&fixture);
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
    let mut products = DescriptorRuleProducts::default();
    let mut collector = ValidationCollector::default();
    let mut attachment_count = 0;
    for key in ["StringValueType.ValueType", "Child"] {
        let diagnosis =
            ExtendsLineageDiagnosis::assess(&fixture.nodes[key], &roots.type_descriptor)?;
        let lineage = diagnosis.valid_lineage().unwrap();
        attachment_count +=
            ConstraintContributions::resolve_with_reader(lineage, &CurrentDescriptorReader)?
                .effective
                .len();
        assert!(assessment.assess_descriptor(lineage, &mut products, &mut collector)?);
    }
    assert_eq!(collector.observations().constraint_attachment_count, attachment_count);
    assert_eq!(collector.observations().constraint_declaration_count, 2);
    assert_eq!(collector.observations().effective_constraint_count, 0);
    assert!(collector.into_report().is_accepted());
    let subject = fixture.nodes["Child"].clone();
    let findings = dispatch_c2_rule(
        &mut fixture,
        CoreValidationRuleName::InheritedValueConstraintNonRelaxation,
        &products,
        &subject,
    )?;
    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn malformed_effective_state_cannot_remove_or_reattribute_an_inherited_constraint(
) -> Result<(), HolonError> {
    for remove in [true, false] {
        let mut fixture = fixture()?;
        configured(&mut fixture, "Inherited", UNIQUE)?;
        fixture.link(
            "StringValueType.ValueType",
            CoreRelationshipTypeName::Constraints,
            "Inherited",
        )?;
        fixture.node("Child")?;
        fixture.link("Child", CoreRelationshipTypeName::Extends, "StringValueType.ValueType")?;
        let subject = fixture.nodes["Child"].clone();
        let roots = roots(&fixture);
        let diagnosis = ExtendsLineageDiagnosis::assess(&subject, &roots.type_descriptor)?;
        let lineage = diagnosis.valid_lineage().unwrap();
        let mut contributions =
            ConstraintContributions::resolve_with_reader(lineage, &CurrentDescriptorReader)?;
        // Deliberately bypass the kernel's additive construction. Normal authoring
        // cannot express removal or provenance replacement of inherited obligations.
        if remove {
            contributions.effective.clear();
        } else {
            contributions.effective[0].declared_on = subject.clone();
        }
        let mut products = DescriptorRuleProducts::default();
        let mut collector = ValidationCollector::default();
        let mut assessment =
            ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
        assert!(!assessment.assess_contributions(
            lineage,
            &contributions,
            &mut products,
            &mut collector
        )?);
        let findings = dispatch_c2_rule(
            &mut fixture,
            CoreValidationRuleName::InheritedValueConstraintNonRelaxation,
            &products,
            &subject,
        )?;
        assert_eq!(findings.len(), 1);
        assert!(
            matches!(&findings[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONSTRAINT-001")
        );
        assert!(findings[0].message.contains(&fixture.nodes["Inherited"].reference_id_string()));
    }
    Ok(())
}

#[test]
fn declaration_acceptance_does_not_enable_an_unsupported_subject_evaluator(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    let constraint = configured(&mut fixture, "Configured", UNIQUE)?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "Configured",
    )?;
    let roots = roots(&fixture);
    let mut collector = ValidationCollector::default();
    assert!(ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?
        .assess_constraint(&constraint, &mut collector)?);
    assert_eq!(collector.observations().effective_constraint_count, 0);
    assert!(collector.into_report().is_accepted());
    let context = ValueValidationContext::resolve(&fixture.context)?;
    let descriptor =
        ValueDescriptor::from_holon(fixture.nodes["StringValueType.ValueType"].clone());
    let value = BaseValue::StringValue(MapString("value".into()));
    let mut collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &descriptor, value: &value, path: &value_path() },
        &context,
        &mut collector,
    )?;
    assert_eq!(collector.observations().effective_constraint_count, 1);
    assert_eq!(collector.observations().constraint_declaration_count, 0);
    assert!(collector.into_report().violations.iter().any(|finding| matches!(
        finding.kind,
        CommitValidationViolationKind::UnsupportedConstraintType { .. }
    )));
    Ok(())
}

#[test]
fn extension_owned_constraint_and_dependency_owned_reuse_do_not_change_ownership(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    fixture.node("CoreSchema")?;
    fixture.node("ExtensionSchema")?;
    fixture.node("Extension.ConstraintType")?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::Extends,
        "ConstraintType.HolonType",
    )?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::ApplicableToDescriptorTypes,
        "StringValueType.ValueType",
    )?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::ComponentOf,
        "ExtensionSchema",
    )?;
    configured(&mut fixture, "ExtensionRule", "Extension.ConstraintType")?;
    configured(&mut fixture, "ReusableRule", UNIQUE)?;
    fixture.link("ExtensionRule", CoreRelationshipTypeName::RuleOf, "ExtensionSchema")?;
    fixture.link("ReusableRule", CoreRelationshipTypeName::RuleOf, "CoreSchema")?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "ReusableRule",
    )?;
    fixture.node("ExtensionDescriptor")?;
    fixture.link(
        "ExtensionDescriptor",
        CoreRelationshipTypeName::Extends,
        "StringValueType.ValueType",
    )?;
    fixture.link("ExtensionDescriptor", CoreRelationshipTypeName::Constraints, "ExtensionRule")?;
    fixture.link(
        "ExtensionDescriptor",
        CoreRelationshipTypeName::ComponentOf,
        "ExtensionSchema",
    )?;
    let roots = roots(&fixture);
    let diagnosis = ExtendsLineageDiagnosis::assess(
        &fixture.nodes["ExtensionDescriptor"],
        &roots.type_descriptor,
    )?;
    let mut collector = ValidationCollector::default();
    let mut products = DescriptorRuleProducts::default();
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
    assert!(assessment.assess_descriptor(
        diagnosis.valid_lineage().unwrap(),
        &mut products,
        &mut collector
    )?);
    assert_eq!(collector.observations().constraint_declaration_count, 2);
    assert_eq!(collector.observations().effective_constraint_count, 0);
    assert!(collector.into_report().is_accepted());
    let owners = fixture.nodes["ReusableRule"].related_holons(CoreRelationshipTypeName::RuleOf)?;
    let owners = owners.read().unwrap();
    assert_eq!(owners.get_members().len(), 1);
    assert!(same_definition(&owners.get_members()[0], &fixture.nodes["CoreSchema"]));
    Ok(())
}

#[test]
fn invalid_declarations_accumulate_attachment_findings_and_fresh_assessment_accepts_correction(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    let mut constraint = configured(&mut fixture, "BadConfiguration", BOUNDED[0])?;
    // No bounds; the type is also initially inapplicable to this descriptor.
    fixture.link(
        "IntegerValueType.ValueType",
        CoreRelationshipTypeName::Constraints,
        "BadConfiguration",
    )?;
    let roots = roots(&fixture);
    let subject = fixture.nodes["IntegerValueType.ValueType"].clone();
    let diagnosis = ExtendsLineageDiagnosis::assess(&subject, &roots.type_descriptor)?;
    let mut collector = ValidationCollector::default();
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
    assert!(!assessment.assess_descriptor(
        diagnosis.valid_lineage().unwrap(),
        &mut DescriptorRuleProducts::default(),
        &mut collector
    )?);
    assert!(!assessment.assess_constraint(&constraint, &mut collector)?);
    assert_eq!(collector.observations().constraint_declaration_count, 1);
    let report = collector.into_report();
    for code in ["DS-CONSTRAINT-002", "DS-CONSTRAINT-003"] {
        assert!(report.violations.iter().any(|finding|
            matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code: actual } if actual == code)));
    }
    assert!(report.violations.iter().any(|finding| matches!(
        finding.kind,
        CommitValidationViolationKind::UnresolvedLocalDependency
    )));
    drop(assessment);
    constraint
        .with_property_value("Minimum", 0_i64)?
        .with_property_value("MinimumIsInclusive", true)?;
    fixture.link(
        BOUNDED[0],
        CoreRelationshipTypeName::ApplicableToDescriptorTypes,
        "IntegerValueType.ValueType",
    )?;
    let mut collector = ValidationCollector::default();
    assert!(ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?
        .assess_descriptor(
            diagnosis.valid_lineage().unwrap(),
            &mut DescriptorRuleProducts::default(),
            &mut collector
        )?);
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn malformed_describing_selections_are_findings_but_reader_failures_stay_operational(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    let missing = fixture.node("Missing")?;
    let mut multiple = fixture.node("Multiple")?;
    // Author intentionally ambiguous input in one ungoverned operation, before
    // any describing contract becomes authoritative for later mutations.
    multiple.add_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![fixture.nodes[UNIQUE].clone(), fixture.nodes[CARDINALITY].clone()],
    )?;
    let wrong = configured(&mut fixture, "Wrong", "StringValueType.ValueType")?;
    let roots = roots(&fixture);
    let mut collector = ValidationCollector::default();
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
    for constraint in [&missing, &multiple, &wrong] {
        assert!(!assessment.assess_constraint(constraint, &mut collector)?);
    }
    let report = collector.into_report();
    assert_eq!(
        report
            .violations
            .iter()
            .filter(|finding| matches!(finding.kind, CommitValidationViolationKind::NoDescriptor))
            .count(),
        2
    );
    assert!(report.violations.iter().any(|finding| finding.message.contains("MissingDescribedBy")));
    assert!(report
        .violations
        .iter()
        .any(|finding| finding.message.contains("MultipleDescribedBy")));

    struct Unreadable {
        subject: Option<HolonReference>,
    }
    impl DescriptorReader for Unreadable {
        type Error = HolonError;
        fn select(&self, reference: &HolonReference) -> Result<HolonReference, HolonError> {
            if self.subject.as_ref().is_none_or(|subject| same_definition(subject, reference)) {
                return Err(HolonError::EmptyField("reader failed".into()));
            }
            Ok(reference.clone())
        }
        fn operational_error(error: &HolonError) -> Option<&HolonError> {
            Some(error)
        }
    }
    let collector = ValidationCollector::default();
    assert!(matches!(
        ConstraintDeclarationAssessment::new(&roots, &Unreadable { subject: None }),
        Err(HolonError::EmptyField(_))
    ));
    assert_eq!(collector.observations().constraint_declaration_count, 0);
    let reader = Unreadable { subject: Some(missing.clone()) };
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &reader)?;
    let mut collector = ValidationCollector::default();
    assert!(matches!(
        assessment.assess_constraint(&missing, &mut collector),
        Err(HolonError::EmptyField(_))
    ));
    assert_eq!(collector.observations().constraint_declaration_count, 0);
    Ok(())
}

#[test]
fn prospective_declaration_reads_replacement_configuration_and_deduplicates_saved_references(
) -> Result<(), HolonError> {
    use holons_core::descriptors::ProspectiveDescriptorReader;
    let mut fixture = fixture()?;
    let mut constraint = configured(&mut fixture, "Configured", CARDINALITY)?;
    constraint.with_property_value("Minimum", 0_i64)?;
    let fixture = fixture.saved_snapshot()?;
    let mut replacement = fixture.replacement("Configured")?;
    replacement.with_property_value("Minimum", -1_i64)?;
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &[replacement.clone()])?;
    let roots = roots(&fixture);
    let mut collector = ValidationCollector::default();
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &reader).unwrap();
    assert!(!assessment.assess_constraint(&fixture.nodes["Configured"], &mut collector).unwrap());
    assert!(!assessment.assess_constraint(&replacement.into(), &mut collector).unwrap());
    assert_eq!(collector.observations().constraint_declaration_count, 1);
    assert_eq!(collector.into_report().violation_count(), 1);
    Ok(())
}

#[test]
fn prospective_parameter_membership_uses_the_replacement_constraint_type() -> Result<(), HolonError>
{
    use holons_core::descriptors::ProspectiveDescriptorReader;
    let mut fixture = fixture()?;
    fixture.node("Extension.ConstraintType")?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::Extends,
        "ConstraintType.HolonType",
    )?;
    fixture.node("Limit.ConfigurationProperty")?;
    fixture
        .nodes
        .get_mut("Limit.ConfigurationProperty")
        .unwrap()
        .with_property_value("TypeName", "Limit")?;
    let mut constraint = configured(&mut fixture, "Configured", "Extension.ConstraintType")?;
    constraint.with_property_value("Limit", 4_i64)?;
    for (child, parent) in [
        ("MetaTypeDescriptor.HolonType", "HolonType.TypeDescriptor"),
        ("MetaHolonType.MetaTypeDescriptor", "MetaTypeDescriptor.HolonType"),
        ("MetaConstraintType.MetaHolonType", "MetaHolonType.MetaTypeDescriptor"),
    ] {
        fixture.node(child)?;
        fixture.link(child, CoreRelationshipTypeName::Extends, parent)?;
    }
    declare_relationship(
        &mut fixture,
        "MetaConstraintType.MetaHolonType",
        "InstanceProperties",
        "PropertyType.TypeDescriptor",
    )?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::DescribedBy,
        "MetaConstraintType.MetaHolonType",
    )?;
    let fixture = fixture.saved_snapshot()?;
    let roots = roots(&fixture);
    let mut collector = ValidationCollector::default();
    assert!(!ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?
        .assess_constraint(&fixture.nodes["Configured"], &mut collector)?);

    let mut replacement = fixture.replacement("Extension.ConstraintType")?;
    replacement.add_related_holons(
        CoreRelationshipTypeName::InstanceProperties,
        vec![fixture.nodes["Limit.ConfigurationProperty"].clone()],
    )?;
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &[replacement])?;
    let mut collector = ValidationCollector::default();
    assert!(ConstraintDeclarationAssessment::new(&roots, &reader)
        .unwrap()
        .assess_constraint(&fixture.nodes["Configured"], &mut collector)
        .unwrap());
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn bound_diagnostics_distinguish_negative_integers_from_wrong_value_kinds() -> Result<(), HolonError>
{
    for (value, detail) in [
        (BaseValue::IntegerValue(MapInteger(-1)), "found integer -1"),
        (BaseValue::StringValue(MapString("abc".into())), "found value kind String"),
    ] {
        let mut fixture = fixture()?;
        let mut constraint = configured(&mut fixture, "Configured", BOUNDED[0])?;
        constraint
            .with_property_value("Minimum", value)?
            .with_property_value("MinimumIsInclusive", true)?;
        let roots = roots(&fixture);
        let mut collector = ValidationCollector::default();
        assert!(!ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?
            .assess_constraint(&constraint, &mut collector)?);
        let report = collector.into_report();
        assert_eq!(report.violation_count(), 1);
        assert!(report.violations[0].message.contains("Minimum must"));
        assert!(report.violations[0].message.contains(detail));
    }
    Ok(())
}

#[test]
fn malformed_inclusivity_does_not_invent_an_equal_endpoint_interval_failure(
) -> Result<(), HolonError> {
    let mut fixture = fixture()?;
    let mut constraint = configured(&mut fixture, "Configured", BOUNDED[0])?;
    constraint
        .with_property_value("Minimum", 2_i64)?
        .with_property_value("Maximum", 2_i64)?
        .with_property_value("MinimumIsInclusive", "invalid")?
        .with_property_value("MaximumIsInclusive", true)?;
    let roots = roots(&fixture);
    let mut collector = ValidationCollector::default();
    assert!(!ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?
        .assess_constraint(&constraint, &mut collector)?);
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(report.violations[0].message.contains("MinimumIsInclusive must be Boolean"));
    Ok(())
}

#[test]
fn fixed_family_roots_are_selected_once_for_the_declaration_scope() -> Result<(), HolonError> {
    use std::cell::RefCell;
    struct CountingReader {
        family_roots: Vec<HolonReference>,
        selections: RefCell<[usize; 6]>,
    }
    impl DescriptorReader for CountingReader {
        type Error = HolonError;
        fn select(&self, reference: &HolonReference) -> Result<HolonReference, HolonError> {
            if let Some(index) =
                self.family_roots.iter().position(|root| same_definition(root, reference))
            {
                self.selections.borrow_mut()[index] += 1;
            }
            Ok(reference.clone())
        }
        fn operational_error(error: &HolonError) -> Option<&HolonError> {
            Some(error)
        }
    }
    let mut fixture = fixture()?;
    fixture.node("Extension.ConstraintType")?;
    fixture.link(
        "Extension.ConstraintType",
        CoreRelationshipTypeName::Extends,
        "ConstraintType.HolonType",
    )?;
    let first = configured(&mut fixture, "First", "Extension.ConstraintType")?;
    let second = configured(&mut fixture, "Second", "Extension.ConstraintType")?;
    let roots = roots(&fixture);
    let reader = CountingReader {
        family_roots: roots
            .bounded
            .iter()
            .chain([&roots.cardinality, &roots.unique_items])
            .cloned()
            .collect(),
        selections: RefCell::new([0; 6]),
    };
    let mut assessment = ConstraintDeclarationAssessment::new(&roots, &reader)?;
    let mut collector = ValidationCollector::default();
    for constraint in [&first, &second, &first] {
        assert!(assessment.assess_constraint(constraint, &mut collector)?);
    }
    assert_eq!(*reader.selections.borrow(), [1; 6]);
    assert_eq!(collector.observations().constraint_declaration_count, 2);
    Ok(())
}

#[test]
fn shared_invalid_constraint_findings_keep_their_subject_in_either_candidate_order(
) -> Result<(), HolonError> {
    for order in [["First", "Second"], ["Second", "First"]] {
        let mut fixture = fixture()?;
        let constraint = configured(&mut fixture, "SharedInvalid", BOUNDED[0])?;
        for key in order {
            fixture.node(key)?;
            fixture.link(key, CoreRelationshipTypeName::Extends, "StringValueType.ValueType")?;
            fixture.link(key, CoreRelationshipTypeName::Constraints, "SharedInvalid")?;
        }
        let roots = roots(&fixture);
        let mut assessment =
            ConstraintDeclarationAssessment::new(&roots, &CurrentDescriptorReader)?;
        let mut collector = ValidationCollector::default();
        for key in order {
            let diagnosis =
                ExtendsLineageDiagnosis::assess(&fixture.nodes[key], &roots.type_descriptor)?;
            assert!(!assessment.assess_descriptor(
                diagnosis.valid_lineage().unwrap(),
                &mut DescriptorRuleProducts::default(),
                &mut collector
            )?);
        }
        let report = collector.into_report();
        let configuration_findings: Vec<_> = report.violations.iter().filter(|finding|
            matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONSTRAINT-003")).collect();
        assert_eq!(configuration_findings.len(), 1);
        assert_eq!(
            configuration_findings[0].subject,
            ValidationSubjectPath::Holon { holon_identity: constraint.reference_id_string() }
        );
    }
    Ok(())
}
