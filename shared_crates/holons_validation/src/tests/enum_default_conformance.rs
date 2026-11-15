use super::support::*;
use crate::{assess_commit_candidates, validate_commit_candidates};
use base_types::{BaseValue, MapEnumValue};
use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_core::{HolonReference, StagedReference, WritableHolon};
use type_names::{CoreRelationshipTypeName as R, CoreValidationRuleName as Rule};

fn enum_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = core_fixture()?;
    fixture
        .node("DeclaredRelationshipType.RelationshipType")?
        .with_property_value("TypeName", "DeclaredRelationshipType")?;

    let root = fixture.nodes["TypeDescriptor"].clone();
    fixture
        .node("MetaEnumValueType.MetaValueType")?
        .remove_related_holons(R::Extends, vec![root])?;
    fixture.link(
        "MetaEnumValueType.MetaValueType",
        R::Extends,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    fixture.node("Color")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("Color", R::Extends, "EnumValueType.ValueType")?;
    fixture
        .node("red-key")?
        .with_property_value("TypeName", "Red")?
        .with_property_value("DisplayName", "Bright red")?;
    fixture.link("Color", R::Variants, "red-key")?;
    let old = fixture.nodes["StringValueType.ValueType"].clone();
    fixture.node("Title.PropertyType")?.remove_related_holons(R::ValueType, vec![old])?;
    fixture.link("Title.PropertyType", R::ValueType, "Color")?;
    fixture
        .node("Variants.Relationship")?
        .with_property_value("TypeName", "Variants")?
        .with_property_value("AllowsDuplicates", false)?
        .with_property_value("IsOrdered", false)?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "Variants.Relationship",
        R::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link("Variants.Relationship", R::SourceType, "HolonType.TypeDescriptor")?;
    fixture.link("Variants.Relationship", R::TargetType, "HolonType.TypeDescriptor")?;
    fixture.link(
        "MetaEnumValueType.MetaValueType",
        R::InstanceRelationships,
        "Variants.Relationship",
    )?;
    fixture.link("EnumValueType.ValueType", R::DescribedBy, "MetaEnumValueType.MetaValueType")?;
    fixture.link("Color", R::ComponentOf, "CoreOwner")?;
    fixture.link("Color", R::DescribedBy, "MetaEnumValueType.MetaValueType")?;
    Ok(fixture)
}

fn candidate(fixture: &Fixture, key: &str) -> StagedReference {
    let HolonReference::Staged(value) = fixture.nodes[key].clone() else {
        panic!("staged fixture")
    };
    value
}

fn token(value: &str) -> BaseValue {
    BaseValue::EnumValue(MapEnumValue(value.into()))
}

fn findings(assessment: &crate::CommitAssessment, code: &str) -> usize {
    assessment.report.violations.iter().filter(|finding|
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code: actual } if actual == code)
    ).count()
}

#[test]
fn exact_enum_tokens_and_substitutes_dispatch_membership() -> Result<(), HolonError> {
    let fixture = enum_fixture()?;
    let mut subject = fixture.staged_subject("consumer")?;
    for (value, accepted) in [
        ("Red", true),
        ("red-key", false),
        ("red-key.Color", false),
        ("Bright red", false),
        ("red", false),
        ("RED", false),
        ("undeclared", false),
    ] {
        subject.with_property_value("Title", token(value))?;
        let assessment = assess_commit_candidates(&fixture.context, &[subject.clone()])?;
        assert_eq!(assessment.report.is_accepted(), accepted, "{:?}", assessment.report);
        assert_eq!(findings(&assessment, "DS-ENUM-002"), usize::from(!accepted));
        assert!(assessment
            .observations
            .dispatched_rule_keys
            .contains(Rule::EnumTokenMembership.as_str()));
    }
    Ok(())
}

#[test]
fn tokens_can_equal_keys_or_labels_but_are_never_normalized() -> Result<(), HolonError> {
    let mut fixture = enum_fixture()?;
    let mut subject = fixture.staged_subject("consumer")?;
    for canonical in ["red-key", "Bright red", "Café"] {
        fixture.node("red-key")?.with_property_value("TypeName", canonical)?;
        subject.with_property_value("Title", token(canonical))?;
        assert!(assess_commit_candidates(&fixture.context, &[subject.clone()])?
            .report
            .is_accepted());
    }
    subject.with_property_value("Title", token("Cafe\u{301}"))?;
    assert_eq!(
        findings(&assess_commit_candidates(&fixture.context, &[subject])?, "DS-ENUM-002"),
        1
    );
    Ok(())
}

#[test]
fn duplicate_tokens_preserve_claimants_and_definition_dispatch() -> Result<(), HolonError> {
    let mut fixture = enum_fixture()?;
    fixture.node("other-red")?.with_property_value("TypeName", "Red")?;
    fixture.link("Color", R::Variants, "other-red")?;
    let mut subject = fixture.staged_subject("consumer")?;
    subject.with_property_value("Title", token("Red"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Color"), subject])?;
    assert_eq!(findings(&assessment, "DS-ENUM-001"), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-ENUM-002"), 1);
    for rule in [Rule::EnumMemberNamesUnique, Rule::EnumTokenMembership] {
        assert!(assessment.observations.dispatched_rule_keys.contains(rule.as_str()));
    }
    let definition = assessment
        .report
        .violations
        .iter()
        .find(|f| f.rule_key.as_deref() == Some(Rule::EnumMemberNamesUnique.as_str()))
        .unwrap();
    for key in ["red-key", "other-red", "Color"] {
        let identity =
            holons_core::DefinitionIdentity::for_reference(&fixture.nodes[key], &fixture.context)?;
        assert!(definition.message.contains(&identity.to_string()), "{}", definition.message);
    }
    Ok(())
}

#[test]
fn abstract_variant_is_a_definition_defect_and_never_a_token() -> Result<(), HolonError> {
    let mut fixture = enum_fixture()?;
    let anchor = "EnumVariantValueType.ValueType";
    fixture
        .node(anchor)?
        .with_property_value("IsAbstractType", true)?
        .with_property_value("DefinesInstanceTypeKind", true)?
        .with_property_value("TypeName", "EnumVariantValueType")?;
    fixture.link(anchor, R::Extends, "ValueType.TypeDescriptor")?;
    fixture.link("Color", R::Variants, anchor)?;
    let mut subject = fixture.staged_subject("consumer")?;
    subject.with_property_value("Title", token("EnumVariantValueType"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Color"), subject])?;
    assert_eq!(findings(&assessment, "DS-ENUM-001"), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-ENUM-002"), 1);
    let identity =
        holons_core::DefinitionIdentity::for_reference(&fixture.nodes[anchor], &fixture.context)?;
    assert!(assessment
        .report
        .violations
        .iter()
        .any(|f| f.rule_key.as_deref() == Some(Rule::EnumMemberNamesUnique.as_str())
            && f.message.contains(&identity.to_string())));
    Ok(())
}

#[test]
fn sub_enum_does_not_inherit_membership() -> Result<(), HolonError> {
    let mut fixture = enum_fixture()?;
    fixture.node("SubColor")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("SubColor", R::Extends, "Color")?;
    let parent = fixture.nodes["Color"].clone();
    fixture.node("Title.PropertyType")?.remove_related_holons(R::ValueType, vec![parent])?;
    fixture.link("Title.PropertyType", R::ValueType, "SubColor")?;
    let mut subject = fixture.staged_subject("consumer")?;
    subject.with_property_value("Title", token("Red"))?;
    assert_eq!(
        findings(&assess_commit_candidates(&fixture.context, &[subject])?, "DS-ENUM-002"),
        1
    );
    Ok(())
}

#[test]
fn malformed_enum_product_becomes_attributed_dependency_findings() -> Result<(), HolonError> {
    let mut fixture = enum_fixture()?;
    fixture.node("red-key")?.remove_property_value("TypeName")?;
    let mut subject = fixture.staged_subject("consumer")?;
    subject.with_property_value("Title", token("Red"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Color"), subject])?;
    for rule in [Rule::EnumMemberNamesUnique, Rule::EnumTokenMembership] {
        assert!(
            assessment
                .report
                .violations
                .iter()
                .any(|f| f.rule_key.as_deref() == Some(rule.as_str())
                    && matches!(f.kind, CommitValidationViolationKind::UnresolvedLocalDependency)),
            "{:?}",
            assessment.report
        );
    }
    Ok(())
}

#[test]
fn enum_correction_replaces_findings_and_acquisition_errors_preserve_outcomes(
) -> Result<(), HolonError> {
    let fixture = enum_fixture()?;
    let mut subject = fixture.staged_subject("consumer")?;
    subject.with_property_value("Title", token("wrong"))?;
    assert!(!validate_commit_candidates(&fixture.context, &[subject.clone()])?.is_accepted());
    assert_eq!(subject.validation_state()?, ValidationState::Invalid);
    subject.with_property_value("Title", token("Red"))?;
    assert!(validate_commit_candidates(&fixture.context, &[subject.clone()])?.is_accepted());
    assert!(subject.validation_findings()?.is_empty());
    let error = HolonError::NotImplemented("enum member acquisition failed".into());
    let fixture = enum_fixture()?.saved_snapshot_with_read_errors(&[("red-key", error.clone())])?;
    let mut subject = fixture.staged_subject("acquisition-failure")?;
    subject.with_property_value("Title", token("Red"))?;
    subject.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
    let before = (subject.validation_state()?, subject.validation_findings()?);
    assert_eq!(
        validate_commit_candidates(&fixture.context, &[subject.clone()]).unwrap_err(),
        error
    );
    assert_eq!((subject.validation_state()?, subject.validation_findings()?), before);
    Ok(())
}

#[test]
fn staged_enum_successors_do_not_redirect_saved_value_contracts() -> Result<(), HolonError> {
    use base_types::ToBaseValue;
    use holons_core::core_shared_objects::WritableRelationship;
    use type_names::ToPropertyName;
    let fixture = enum_fixture()?.saved_snapshot()?;
    let variant = fixture.stage_update_of_with("red-key", |model| {
        model.properties.insert("TypeName".to_property_name(), "Crimson".to_base_value());
        Ok(())
    })?;
    let successor = fixture.stage_update_of_with("Color", |model| {
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        relationships.map.remove(&R::Variants.as_relationship_name());
        relationships.add_related_holons_with_keys(
            R::Variants.as_relationship_name(),
            vec![(variant.clone().into(), None)],
        )?;
        model.relationships = Some(relationships.into());
        Ok(())
    })?;
    let mut subject = fixture.staged_subject("old-contract")?;
    subject.with_property_value("Title", token("Red"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[subject.clone(), successor.clone()])?;
    assert!(assessment.report.is_accepted(), "{:?}", assessment.report);
    subject.with_property_value("Title", token("Crimson"))?;
    assert_eq!(
        findings(&assess_commit_candidates(&fixture.context, &[subject])?, "DS-ENUM-002"),
        1
    );

    let property = fixture.stage_update_of_with("Title.PropertyType", |model| {
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        relationships.map.remove(&R::ValueType.as_relationship_name());
        relationships.add_related_holons_with_keys(
            R::ValueType.as_relationship_name(),
            vec![(successor.clone().into(), None)],
        )?;
        model.relationships = Some(relationships.into());
        Ok(())
    })?;
    let contract = fixture.stage_update_of_with("Contract", |model| {
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        // Preserve Key while explicitly adopting the successor property definition.
        relationships.map.remove(&R::InstanceProperties.as_relationship_name());
        relationships.add_related_holons_with_keys(
            R::InstanceProperties.as_relationship_name(),
            vec![
                (property.clone().into(), None),
                (fixture.nodes["Key.PropertyType"].clone(), None),
            ],
        )?;
        model.relationships = Some(relationships.into());
        Ok(())
    })?;
    let transient = fixture.context.mutation().new_holon(Some("new-contract".into()))?;
    let mut subject = fixture.context.mutation().stage_new_holon(transient)?;
    subject.with_descriptor(contract.into())?;
    subject.with_property_value("Title", token("Red"))?;
    assert_eq!(
        findings(&assess_commit_candidates(&fixture.context, &[subject.clone()])?, "DS-ENUM-002"),
        1
    );
    subject.with_property_value("Title", token("Crimson"))?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
    assert!(assessment.report.is_accepted(), "{:?}", assessment.report);
    Ok(())
}

#[test]
fn enum_membership_does_not_duplicate_native_kind_failures() -> Result<(), HolonError> {
    let fixture = enum_fixture()?;
    let mut subject = fixture.staged_subject("wrong-kind")?;
    subject.with_property_value("Title", 42_i64)?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "BaseValueKindMismatch"), 1);
    assert_eq!(findings(&assessment, "DS-ENUM-002"), 0);
    Ok(())
}

#[test]
fn enum_handlers_consume_sealed_products_without_reads_or_derivation() -> Result<(), HolonError> {
    for saved in [false, true] {
        let fixture = enum_fixture()?;
        let fixture = if saved { fixture.saved_snapshot()? } else { fixture };
        let mut subject = fixture.staged_subject("consumer")?;
        subject.with_property_value("Title", token("Red"))?;
        let definition = if saved {
            fixture.stage_update_of_with("Color", |model| {
                use base_types::ToBaseValue;
                use type_names::ToPropertyName;
                model
                    .properties
                    .insert("TypeName".to_property_name(), "ColorSuccessor".to_base_value());
                Ok(())
            })?
        } else {
            candidate(&fixture, "Color")
        };
        let candidates = [subject, definition];
        let constructed = crate::assessment::commit::construct_assessment(
            &fixture.context,
            &candidates,
            crate::ValidationCollector::default(),
        )?;
        let reads = (fixture.backend_node_reads(), fixture.backend_relationship_reads());
        let probe =
            holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
        constructed.assess(|_| {})?;
        assert_eq!(reads, (fixture.backend_node_reads(), fixture.backend_relationship_reads()));
        assert!(probe.finish().0.values().all(|count| *count == 0));
    }
    Ok(())
}

/// Models the ordinary AnyBaseValue property and the dependent declaration contract together.
fn default_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = enum_fixture()?;
    for name in [
        "ValueType",
        "Extends",
        "ComponentOf",
        "InstanceProperties",
        "InstanceRelationships",
        "ValidationBindings",
        "Predecessor",
    ] {
        fixture
            .node(&format!("{name}.CoreMetadataRelationship"))?
            .with_property_value("AllowsDuplicates", false)?
            .with_property_value("IsOrdered", false)?;
    }
    fixture
        .node("Constraints.CoreMetadataRelationship")?
        .with_property_value("TypeName", "Constraints")?
        .with_property_value("DefinesInstanceTypeKind", false)?
        .with_property_value("AllowsDuplicates", false)?
        .with_property_value("IsOrdered", false)?;
    fixture.link(
        "Constraints.CoreMetadataRelationship",
        R::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link(
        "Constraints.CoreMetadataRelationship",
        R::SourceType,
        "MetaTypeDescriptor.HolonType",
    )?;
    fixture.link(
        "Constraints.CoreMetadataRelationship",
        R::TargetType,
        "ConstraintType.HolonType",
    )?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        R::InstanceRelationships,
        "Constraints.CoreMetadataRelationship",
    )?;
    let root = fixture.nodes["TypeDescriptor"].clone();
    fixture
        .node("MetaPropertyType.MetaTypeDescriptor")?
        .remove_related_holons(R::Extends, vec![root])?;
    fixture.link(
        "MetaPropertyType.MetaTypeDescriptor",
        R::Extends,
        "MetaTypeDescriptor.HolonType",
    )?;
    fixture.link("BaseValueValueType.ValueType", R::Extends, "ValueType.TypeDescriptor")?;
    fixture
        .node("DefaultValue.PropertyType")?
        .with_property_value("TypeName", "DefaultValue")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("DefaultValue.PropertyType", R::Extends, "PropertyType.TypeDescriptor")?;
    fixture.link("DefaultValue.PropertyType", R::ValueType, "BaseValueValueType.ValueType")?;
    fixture.link(
        "MetaPropertyType.MetaTypeDescriptor",
        R::InstanceProperties,
        "DefaultValue.PropertyType",
    )?;
    let old = fixture.nodes["MetaHolonType.MetaTypeDescriptor"].clone();
    fixture
        .node("PropertyType.TypeDescriptor")?
        .remove_related_holons(R::DescribedBy, vec![old])?;
    fixture.link(
        "PropertyType.TypeDescriptor",
        R::DescribedBy,
        "MetaPropertyType.MetaTypeDescriptor",
    )?;
    fixture.link("Title.PropertyType", R::ComponentOf, "CoreOwner")?;
    fixture.link("Title.PropertyType", R::DescribedBy, "MetaPropertyType.MetaTypeDescriptor")?;
    Ok(fixture)
}

fn select_default_type(fixture: &mut Fixture, target: &str) -> Result<(), HolonError> {
    let old = fixture.nodes["Color"].clone();
    fixture.node("Title.PropertyType")?.remove_related_holons(R::ValueType, vec![old])?;
    fixture.link("Title.PropertyType", R::ValueType, target)
}

#[test]
fn unused_property_defaults_require_requiredness_and_dispatch_independently(
) -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", token("Red"))?;
    for required in [true, false] {
        fixture.node("Title.PropertyType")?.with_property_value("IsValueRequired", required)?;
        let assessment = assess_commit_candidates(
            &fixture.context,
            &[candidate(&fixture, "Title.PropertyType")],
        )?;
        assert_eq!(
            assessment.report.violation_count(),
            usize::from(!required),
            "{:?}",
            assessment.report
        );
        assert_eq!(findings(&assessment, "DS-DEFAULT-001"), usize::from(!required));
        for rule in [Rule::DefaultsRequireRequiredProperties, Rule::DefaultValueConformance] {
            assert!(assessment.observations.dispatched_rule_keys.contains(rule.as_str()));
        }
    }
    fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", 12_i64)?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(assessment.report.violation_count(), 2, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-DEFAULT-001"), 1);
    assert_eq!(findings(&assessment, "BaseValueKindMismatch"), 1);
    Ok(())
}

#[test]
fn enum_defaults_use_membership_without_duplicate_ordinary_property_findings(
) -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", token("undeclared"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-ENUM-002"), 1);
    assert_eq!(
        assessment.report.violations[0].rule_key.as_deref(),
        Some(Rule::EnumTokenMembership.as_str())
    );
    assert_eq!(
        assessment.report.violations[0].subject,
        core_types::ValidationSubjectPath::Value {
            holon_identity: fixture.nodes["Title.PropertyType"].full_reference_id_string(),
            property: "DefaultValue".into(),
        }
    );
    Ok(())
}

#[test]
fn default_constraints_preserve_declaration_attribution_and_contributing_provenance(
) -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture.node("NarrowString")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.node("WideString")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("WideString", R::Extends, "StringValueType.ValueType")?;
    fixture.link("NarrowString", R::Extends, "WideString")?;
    bounded_constraint(
        &mut fixture,
        "InheritedLength",
        "StringLengthConstraint.ConstraintType",
        "WideString",
        None,
        Some(8),
    )?;
    bounded_constraint(
        &mut fixture,
        "TighterLength",
        "StringLengthConstraint.ConstraintType",
        "NarrowString",
        None,
        Some(2),
    )?;
    select_default_type(&mut fixture, "NarrowString")?;
    fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", "long")?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(assessment.observations.evaluated_value_constraint_count, 2);
    let finding = &assessment.report.violations[0];
    assert!(matches!(&finding.kind, CommitValidationViolationKind::ConstraintViolation {
        constraint_identity, constraint_type_identity, ..
    } if constraint_identity == &fixture.nodes["TighterLength"].reference_id_string()
        && constraint_type_identity == &fixture.nodes["StringLengthConstraint.ConstraintType"].reference_id_string()));
    assert_eq!(finding.rule_key.as_deref(), Some(Rule::DefaultValueConformance.as_str()));
    assert_eq!(
        finding.descriptor_identity,
        Some(fixture.nodes["NarrowString"].reference_id_string())
    );
    assert_eq!(
        finding.subject,
        core_types::ValidationSubjectPath::Value {
            holon_identity: fixture.nodes["Title.PropertyType"].full_reference_id_string(),
            property: "DefaultValue".into(),
        }
    );
    Ok(())
}

#[test]
fn populated_array_defaults_reject_with_and_without_unsupported_constraints(
) -> Result<(), HolonError> {
    for attached in [false, true] {
        let mut fixture = default_fixture()?;
        fixture.link("ValueArrayValueType.ValueType", R::Extends, "ValueType.TypeDescriptor")?;
        select_default_type(&mut fixture, "ValueArrayValueType.ValueType")?;
        if attached {
            bounded_constraint(
                &mut fixture,
                "Items",
                "ItemCountConstraint.ConstraintType",
                "ValueArrayValueType.ValueType",
                None,
                Some(2),
            )?;
        }
        fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", "populated")?;
        let assessment = assess_commit_candidates(
            &fixture.context,
            &[candidate(&fixture, "Title.PropertyType")],
        )?;
        assert_eq!(
            findings(&assessment, "UnsupportedValueRepresentation"),
            1,
            "{:?}",
            assessment.report
        );
        assert_eq!(assessment.report.violation_count(), 1 + usize::from(attached));
        if attached {
            assert!(assessment.report.violations.iter().any(|f| matches!(
                f.kind,
                CommitValidationViolationKind::UnsupportedConstraintType { .. }
            )));
        }
    }
    Ok(())
}

#[test]
fn saved_default_definitions_are_supporting_inputs_and_never_supply_omitted_values(
) -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture
        .node("Title.PropertyType")?
        .with_property_value("IsValueRequired", false)?
        .with_property_value("DefaultValue", token("undeclared"))?;
    let fixture = fixture.saved_snapshot()?;
    let mut subject = fixture.staged_subject("saved-support")?;
    subject.with_property_value("Title", token("Red"))?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
    assert!(assessment.report.is_accepted(), "{:?}", assessment.report);
    assert!(!assessment
        .observations
        .dispatched_rule_keys
        .contains(Rule::DefaultValueConformance.as_str()));

    let mut fixture = default_fixture()?;
    fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", token("Red"))?;
    let mut subject = fixture.staged_subject("explicit-omission")?;
    subject.remove_property_value("Title")?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject.clone()])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-PROP-001"), 1);
    use holons_core::ReadableHolon;
    assert!(subject.property_value("Title")?.is_none());
    Ok(())
}

#[test]
fn default_handlers_consume_sealed_products_without_reads_or_derivation() -> Result<(), HolonError>
{
    for saved in [false, true] {
        let mut fixture = default_fixture()?;
        fixture.node("Title.PropertyType")?.with_property_value("DefaultValue", token("Red"))?;
        let fixture = if saved { fixture.saved_snapshot()? } else { fixture };
        let property = if saved {
            fixture.stage_update_of("Title.PropertyType")?
        } else {
            candidate(&fixture, "Title.PropertyType")
        };
        let candidates = [property];
        let constructed = crate::assessment::commit::construct_assessment(
            &fixture.context,
            &candidates,
            crate::ValidationCollector::default(),
        )?;
        let reads = (fixture.backend_node_reads(), fixture.backend_relationship_reads());
        let probe =
            holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
        let (prepared, observations) = constructed.assess(|_| {})?;
        let report = prepared.into_report();
        assert!(report.is_accepted(), "{report:?}");
        assert!(observations.dispatched_rule_keys.contains(Rule::DefaultValueConformance.as_str()));
        assert_eq!(reads, (fixture.backend_node_reads(), fixture.backend_relationship_reads()));
        assert!(probe.finish().0.values().all(|count| *count == 0));
    }
    Ok(())
}

#[test]
fn missing_defaults_are_valid_and_unusable_requiredness_is_attributed() -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture.node("Title.PropertyType")?.with_property_value("IsValueRequired", false)?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert!(assessment.report.is_accepted(), "{:?}", assessment.report);
    fixture
        .node("Title.PropertyType")?
        .with_property_value("IsValueRequired", "malformed")?
        .with_property_value("DefaultValue", token("Red"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(findings(&assessment, "DS-DEFAULT-001"), 0);
    let dependencies: Vec<_> = assessment
        .report
        .violations
        .iter()
        .filter(|f| f.rule_key.as_deref() == Some(Rule::DefaultsRequireRequiredProperties.as_str()))
        .collect();
    assert_eq!(dependencies.len(), 1, "{:?}", assessment.report);
    assert_eq!(dependencies[0].kind, CommitValidationViolationKind::UnresolvedLocalDependency);
    assert!(matches!(dependencies[0].subject, core_types::ValidationSubjectPath::Holon { .. }));
    assert_eq!(findings(&assessment, "BaseValueKindMismatch"), 1);
    Ok(())
}

#[test]
fn default_with_missing_value_type_has_an_attributed_blocker() -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    let old = fixture.nodes["Color"].clone();
    fixture
        .node("Title.PropertyType")?
        .remove_related_holons(R::ValueType, vec![old])?
        .with_property_value("DefaultValue", token("Red"))?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(
        assessment.report.violations[0].kind,
        CommitValidationViolationKind::UnresolvedLocalDependency
    );
    assert_eq!(
        assessment.report.violations[0].rule_key.as_deref(),
        Some(Rule::DefaultValueConformance.as_str())
    );
    Ok(())
}

#[test]
fn unusable_default_value_lineage_blocks_only_dependent_assessment() -> Result<(), HolonError> {
    let mut fixture = default_fixture()?;
    fixture.node("BrokenValue")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("BrokenValue", R::Extends, "BrokenValue")?;
    select_default_type(&mut fixture, "BrokenValue")?;
    fixture
        .node("Title.PropertyType")?
        .with_property_value("IsValueRequired", false)?
        .with_property_value("DefaultValue", "populated")?;
    let assessment =
        assess_commit_candidates(&fixture.context, &[candidate(&fixture, "Title.PropertyType")])?;
    assert_eq!(findings(&assessment, "DS-DEFAULT-001"), 1);
    let blockers: Vec<_> = assessment
        .report
        .violations
        .iter()
        .filter(|f| f.rule_key.as_deref() == Some(Rule::DefaultValueConformance.as_str()))
        .collect();
    assert_eq!(blockers.len(), 1, "{:?}", assessment.report);
    assert_eq!(blockers[0].kind, CommitValidationViolationKind::UnresolvedLocalDependency);
    Ok(())
}

#[test]
fn abstract_describer_has_one_conformance_finding() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.link(
        "HolonType.TypeDescriptor",
        R::ValidationBindings,
        Rule::ConcreteDescribingType.as_str(),
    )?;
    fixture.node("Contract")?.with_property_value("IsAbstractType", true)?;
    let mut subject = fixture.staged_subject("abstract-describer")?;
    subject.with_property_value("Title", "present")?;
    let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:?}", assessment.report);
    assert_eq!(findings(&assessment, "DS-CONFORM-001"), 1);
    assert!(assessment
        .observations
        .dispatched_rule_keys
        .contains(Rule::ConcreteDescribingType.as_str()));
    Ok(())
}

#[test]
fn contract_guard_attributes_only_bound_populated_collisions() -> Result<(), HolonError> {
    for bound in [false, true] {
        for (populated, unnamed_member) in [(false, false), (true, false), (true, true)] {
            let mut fixture = readiness_fixture()?;
            fixture
                .node("OtherTitle")?
                .with_property_value("TypeName", "Title")?
                .with_property_value("DefinesInstanceTypeKind", false)?;
            fixture.link("OtherTitle", R::Extends, "PropertyType.TypeDescriptor")?;
            fixture.link("OtherTitle", R::ValueType, "StringValueType.ValueType")?;
            fixture.link("Contract", R::InstanceProperties, "OtherTitle")?;
            if unnamed_member {
                fixture
                    .node("Unnamed")?
                    .with_property_value("TypeName", "")?
                    .with_property_value("DefinesInstanceTypeKind", false)?;
                fixture.link("Unnamed", R::Extends, "PropertyType.TypeDescriptor")?;
                fixture.link("Unnamed", R::ValueType, "StringValueType.ValueType")?;
                fixture.link("Contract", R::InstanceProperties, "Unnamed")?;
            }
            if bound {
                fixture.node(Rule::UniquePropertyMemberBinding.as_str())?;
                fixture.link(
                    Rule::UniquePropertyMemberBinding.as_str(),
                    R::DescribedBy,
                    "HolonValidationRule.HolonType",
                )?;
                fixture.link(
                    "HolonType.TypeDescriptor",
                    R::ValidationBindings,
                    Rule::UniquePropertyMemberBinding.as_str(),
                )?;
            }
            let mut subject = fixture.staged_subject("ambiguous")?;
            if populated {
                subject.with_property_value("Title", 12_i64)?;
            }
            let assessment = assess_commit_candidates(&fixture.context, &[subject])?;
            assert_eq!(
                findings(&assessment, "DS-BIND-001"),
                usize::from(bound && populated),
                "{:?}",
                assessment.report
            );
            assert!(!assessment.report.is_accepted());
            assert_eq!(findings(&assessment, "BaseValueKindMismatch"), 0);
            assert_eq!(
                assessment
                    .observations
                    .dispatched_rule_keys
                    .contains(Rule::UniquePropertyMemberBinding.as_str()),
                bound && populated
            );
            assert_eq!(
                assessment
                    .report
                    .violations
                    .iter()
                    .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency
                        && f.message.contains("ambiguous or unnamed effective members")),
                !bound || !populated || unnamed_member,
                "{:?}",
                assessment.report
            );
            if bound && populated {
                let finding = assessment
                    .report
                    .violations
                    .iter()
                    .find(|f| {
                        f.rule_key.as_deref() == Some(Rule::UniquePropertyMemberBinding.as_str())
                    })
                    .unwrap();
                for key in ["Title.PropertyType", "OtherTitle", "Contract"] {
                    assert!(finding.message.contains(&fixture.nodes[key].reference_id_string()));
                }
            } else {
                assert!(assessment
                    .report
                    .violations
                    .iter()
                    .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
            }
        }
    }
    Ok(())
}
