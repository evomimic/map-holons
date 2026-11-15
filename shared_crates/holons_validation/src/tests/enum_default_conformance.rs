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
