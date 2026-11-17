//! Scenario controls for Commit preparation and interpretation guards.
//!
//! Omitted-versus-required read faults are covered by the active omitted-membership control.
use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::{
    core_shared_objects::{holon::ValidationState, Holon},
    HolonReference, ReadableHolon, WritableHolon,
};
use type_names::{CoreRelationshipTypeName as R, CoreValidationRuleName};

use super::support::{prepare_meta_contract, readiness_fixture, Fixture};
use crate::{
    assessment::commit::{assess_commit_candidates, validate_commit_candidates},
    DescriptorRuleProducts,
};

#[test]
fn unusable_property_selection_beside_invalid_sibling_value() -> Result<(), HolonError> {
    let usable = readiness_fixture()?;
    let mut control = usable.staged_subject("usable-control")?;
    control.with_property_value("Key", 42_i64)?;
    let control_report = validate_commit_candidates(&usable.context, &[control])?;
    assert!(has_rule_code(&control_report.violations, "BaseValueKindMismatch"));
    assert!(has_rule_code(&control_report.violations, "DS-PROP-001"));
    for required in [false, true] {
        let mut fixture = readiness_fixture()?;
        fixture
            .nodes
            .get_mut("Title.PropertyType")
            .unwrap()
            .with_property_value("IsValueRequired", required)?;
        fixture.link("Title.PropertyType", R::ValueType, "BooleanValueType.ValueType")?;
        let fixture = fixture.saved_snapshot()?;
        let mut subject = fixture.staged_subject("unusable-property")?;
        subject.with_property_value("Key", 42_i64)?;
        let report = validate_commit_candidates(&fixture.context, &[subject.clone()])?;
        assert!(!report.is_accepted());
        assert!(has_rule_code(&subject.validation_findings()?, "BaseValueKindMismatch"));
        assert_eq!(has_rule_code(&subject.validation_findings()?, "DS-PROP-001"), required);
        assert!(
            !subject
                .validation_findings()?
                .iter()
                .any(|finding| finding.kind
                    == CommitValidationViolationKind::UnresolvedLocalDependency),
            "absent values require no value interpretation"
        );
        assert_eq!(
            report
                .unattached_findings()?
                .iter()
                .filter(|finding| finding
                    .message
                    .contains("Saved governing contract has invalid effective member definitions"))
                .count(),
            1
        );
        subject.with_property_value("Key", "valid")?;
        validate_commit_candidates(&fixture.context, &[subject.clone()])?;
        assert_eq!(
            subject.validation_state()?,
            if required { ValidationState::Invalid } else { ValidationState::Validated }
        );
        // The very same contract must block value checks once a value is present.
        subject.with_property_value("Title", "present")?;
        validate_commit_candidates(&fixture.context, &[subject.clone()])?;
        assert!(subject.validation_findings()?.iter().any(
            |finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency
        ));
    }
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
        assert!(!subject
            .validation_findings()?
            .iter()
            .any(|f| f.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
        assert!(has_rule_code(&subject.validation_findings()?, "DS-PROP-001"));
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
        let packages = crate::assessment::descriptor_package::DescriptorPackages::construct(
            &fixture.context,
            &[subject.clone().into()],
            &[],
            &crate::assessment::schema_view::SchemaWorkset { schemas: Vec::new() },
        )?;
        let error = packages
            .effective_schema(&subject.clone().into())
            .err()
            .expect("diagnosed structural defect");
        assert!(if cycle {
            matches!(error, HolonError::CyclicExtends { .. })
        } else {
            matches!(error, HolonError::MultipleExtends { .. })
        });
        assert!(matches!(
            packages.constraint_contributions(&subject.clone().into()),
            Err(HolonError::InvalidState(_))
        ));
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
        let mut prod = DescriptorRuleProducts::new(fixture.diagnoses());
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
        assert_eq!(report.violations.len(), 1, "only the independently failing peer rejects");
        assert!(a.validation_findings()?.is_empty());
        assert_eq!(b.validation_findings()?.len(), 1);
        assert!(has_rule_code(&b.validation_findings()?, "DS-PROP-001"));
        assert!(has_rule_code(&report.violations, "DS-PROP-001"));
        assert_eq!(a.validation_state()?, ValidationState::Validated);
        assert_eq!(b.validation_state()?, ValidationState::Invalid);
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

#[test]
fn saved_restoration_attempt_does_not_diagnose_lineage() -> Result<(), HolonError> {
    let mut fixture = Fixture::new()?;
    fixture.node("SavedSupportingDefinition")?;
    let fixture = fixture.saved_snapshot()?;
    let registry = fixture.context.descriptor_registry().unwrap();
    let owner = registry.context()?;
    let mut prepared = owner.mutation().new_holon(None)?;
    prepared.add_related_holons(
        R::DescribedHolonType,
        vec![fixture.nodes["SavedSupportingDefinition"].clone()],
    )?;
    prepared.mark_as_immutable()?;
    // A published incomplete product exercises restoration failure without source diagnosis.
    registry.publish(
        fixture.nodes["SavedSupportingDefinition"].holon_id()?,
        &holons_core::EffectiveHolonDescriptor::from_constructed_holon(prepared.into()),
    )?;
    let probe = holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
    let packages = crate::assessment::descriptor_package::DescriptorPackages::construct(
        &fixture.context,
        &[fixture.nodes["SavedSupportingDefinition"].clone()],
        &[],
        &crate::assessment::schema_view::SchemaWorkset { schemas: Vec::new() },
    )?;
    let observations = probe.finish();
    assert_eq!(observations.0.get("registry_hits"), Some(&1));
    assert_eq!(observations.0.get("saved_restoration_attempts"), Some(&1));
    assert_eq!(observations.0.get("structural_diagnoses").copied().unwrap_or(0), 0);
    assert_eq!(observations.0.get("schema_builder_attempts").copied().unwrap_or(0), 0);
    assert!(matches!(
        packages.effective_schema(&fixture.nodes["SavedSupportingDefinition"]),
        Err(HolonError::InvalidParameter(_))
    ));
    Ok(())
}

#[test]
fn omitted_memberships_do_not_fail_assessment_but_required_reads_do() -> Result<(), HolonError> {
    for (source, edge, required) in [
        ("Contract", R::ValueType, false),
        ("StringValueType.ValueType", R::RuleOf, false),
        // Complete derived payload acquisition still includes this collection.
        ("StringValueType.ValueType", R::InstanceProperties, true),
        ("Title.PropertyType", R::DescribedBy, true),
    ] {
        let fixture = readiness_fixture()?.saved_snapshot()?;
        let mut subject = fixture.staged_subject("demand-read-control")?;
        subject.with_property_value("Title", "valid")?;
        subject.replace_validation_outcome(ValidationState::Validated, Vec::new())?;
        let error = HolonError::InvalidState(format!("injected {source} {edge:?}"));
        fixture.control_saved_relationship_reads(source, edge.clone(), Some(error.clone()))?;
        for report_only in [true, false] {
            let result = if report_only {
                assess_commit_candidates(&fixture.context, &[subject.clone()]).map(|a| a.report)
            } else {
                validate_commit_candidates(&fixture.context, &[subject.clone()])
            };
            if required {
                assert_eq!(result.err(), Some(error.clone()));
            } else {
                assert!(result?.is_accepted());
            }
            assert_eq!(subject.validation_state()?, ValidationState::Validated);
            assert!(subject.validation_findings()?.is_empty());
        }
        assert_eq!(fixture.backend_reads_for_source(source, &edge) > 0, required);
    }
    Ok(())
}

#[test]
fn additional_consumers_share_complete_products_in_either_request_order() -> Result<(), HolonError>
{
    let mut expected = None;
    for reverse in [false, true] {
        let fixture = readiness_fixture()?.saved_snapshot()?;
        let mut roots =
            vec![fixture.nodes["Contract"].clone(), fixture.nodes["Title.PropertyType"].clone()];
        if reverse {
            roots.reverse();
        }
        let probe =
            holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
        let mut index_reads = None;
        let packages =
            crate::assessment::descriptor_package::DescriptorPackages::construct_with_observer(
                &fixture.context,
                &roots,
                &[],
                &crate::assessment::schema_view::SchemaWorkset { schemas: Vec::new() },
                fixture.diagnoses(),
                &mut |event| {
                    if let crate::AssessmentEvent::PreparationOperation {
                        operation: "NameIndexes",
                        starting,
                        ..
                    } = event
                    {
                        if starting {
                            index_reads = Some(fixture.backend_relationship_reads());
                        } else {
                            assert_eq!(
                                index_reads.take(),
                                Some(fixture.backend_relationship_reads()),
                                "indexes consume prepared memberships"
                            );
                        }
                    }
                },
            )?;
        let counts = probe.finish().0;
        assert!(packages.requiredness(&fixture.nodes["Title.PropertyType"])?.unwrap());
        assert!(packages.value_type(&fixture.nodes["Title.PropertyType"])?.is_some());
        let signature = (
            packages.preparation_metrics().constructed_schema_products,
            counts.get("derived_projection_attempts").copied().unwrap_or(0),
            counts.get("schema_builder_attempts").copied().unwrap_or(0),
        );
        assert_eq!(signature.1, 3, "each derived product is projected once");
        if let Some(expected) = &expected {
            assert_eq!(&signature, expected);
        } else {
            expected = Some(signature);
        }
        assert_eq!(packages.preparation_metrics().constructed_schema_products, 4);
        assert!(
            matches!(packages.effective_schema(&fixture.nodes["Schema.HolonType"]), Err(HolonError::InvalidState(message)) if message.contains("Schema") || message.contains("contract consumer"))
        );
    }
    Ok(())
}

#[test]
fn requiredness_and_value_selection_are_concrete_declarations() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .remove_property_value("IsValueRequired")?;
    fixture.nodes.get_mut("Title.PropertyType").unwrap().with_property_value("IsRequired", true)?;
    fixture
        .nodes
        .get_mut("PropertyType.TypeDescriptor")
        .unwrap()
        .with_property_value("IsValueRequired", true)?;
    fixture.node("PropertyMeta")?;
    fixture.link("PropertyMeta", R::Extends, "HolonType.TypeDescriptor")?;
    fixture
        .node("RequirednessDefault")?
        .with_property_value("TypeName", "IsValueRequired")?
        .with_property_value("DefaultValue", false)?;
    fixture.link("PropertyMeta", R::InstanceProperties, "RequirednessDefault")?;
    fixture.link("Title.PropertyType", R::DescribedBy, "PropertyMeta")?;
    let value = fixture.nodes["StringValueType.ValueType"].clone();
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .remove_related_holons(R::ValueType, vec![value])?;
    fixture.link("PropertyType.TypeDescriptor", R::ValueType, "StringValueType.ValueType")?;
    // Neither inherited true nor the alias wins over the governing default.
    let packages = crate::assessment::descriptor_package::DescriptorPackages::construct(
        &fixture.context,
        &[fixture.nodes["Contract"].clone()],
        &[],
        &crate::assessment::schema_view::SchemaWorkset { schemas: Vec::new() },
    )?;
    assert!(!packages.requiredness(&fixture.nodes["Title.PropertyType"])?.unwrap());
    assert!(
        packages.value_type(&fixture.nodes["Title.PropertyType"])?.is_none(),
        "concrete properties do not inherit ValueType selection"
    );
    Ok(())
}

#[test]
fn relationship_endpoints_require_identity_without_target_lineage() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("UnreadEndpoint")?;
    fixture
        .node("UnusedRelationship")?
        .with_property_value("TypeName", "Unused")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("UnusedRelationship", R::Extends, "DeclaredRelationshipType.RelationshipType")?;
    fixture.link("UnusedRelationship", R::SourceType, "Contract")?;
    fixture.link("UnusedRelationship", R::TargetType, "UnreadEndpoint")?;
    fixture.link("Contract", R::InstanceRelationships, "UnusedRelationship")?;
    let fixture = fixture.saved_snapshot()?;
    fixture.control_saved_relationship_reads(
        "UnreadEndpoint",
        R::Extends,
        Some(HolonError::InvalidState("endpoint lineage has no consumer".into())),
    )?;
    let mut subject = fixture.staged_subject("identity-only-endpoint")?;
    subject.with_property_value("Title", "valid")?;
    let report = validate_commit_candidates(&fixture.context, &[subject])?;
    assert!(report.is_accepted(), "{report:?}");
    assert_eq!(fixture.backend_reads_for_source("UnreadEndpoint", &R::Extends), 0);
    Ok(())
}

#[test]
fn unusable_requiredness_preserves_independent_value_checks() -> Result<(), HolonError> {
    for defect in 0..7 {
        let mut fixture = readiness_fixture()?;
        let mut subject = fixture.staged_subject("requiredness-guard")?;
        subject.with_property_value("Key", 42_i64)?;
        let property = fixture.nodes.get_mut("Title.PropertyType").unwrap();
        if defect == 0 {
            property.with_property_value("IsValueRequired", "invalid")?;
        } else {
            property.remove_property_value("IsValueRequired")?;
        }
        if defect >= 2 {
            fixture.node("RequirednessMeta")?;
            fixture.link(
                "RequirednessMeta",
                R::Extends,
                if defect == 6 { "RequirednessMeta" } else { "HolonType.TypeDescriptor" },
            )?;
            let mut describers = vec![fixture.nodes["RequirednessMeta"].clone()];
            if defect == 2 {
                describers.push(fixture.nodes["Contract"].clone());
            }
            fixture
                .nodes
                .get_mut("Title.PropertyType")
                .unwrap()
                .add_related_holons(R::DescribedBy, describers)?;
            if defect >= 4 {
                fixture
                    .node("RequirednessMember")?
                    .with_property_value("TypeName", "IsValueRequired")?;
                fixture.link("RequirednessMeta", R::InstanceProperties, "RequirednessMember")?;
                if defect == 4 {
                    fixture
                        .nodes
                        .get_mut("RequirednessMember")
                        .unwrap()
                        .with_property_value("DefaultValue", "invalid")?;
                }
            }
        }
        let report = validate_commit_candidates(&fixture.context, &[subject])?;
        assert!(
            report.violations.iter().any(|v| v.message.contains("Requiredness is unusable")),
            "case {defect}: {report:?}"
        );
        if defect == 6 {
            assert!(report
                .violations
                .iter()
                .any(|v| v.message.contains("Governing meta-type lineage is unusable")));
        }
        assert!(report.violations.iter().any(|v| v.rule_key.as_deref()
            == Some(CoreValidationRuleName::BaseValueKindMatchesString.as_str())));
    }
    Ok(())
}

/// Enum values and unused default declarations share exact prepared contracts.
fn enum_default_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("PreparedEnum")?.with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link("PreparedEnum", R::Extends, "EnumValueType.ValueType")?;
    fixture.node("PreparedVariant")?.with_property_value("TypeName", "Member")?;
    fixture.link("PreparedEnum", R::Variants, "PreparedVariant")?;
    let old = fixture.nodes["StringValueType.ValueType"].clone();
    fixture.node("Title.PropertyType")?.remove_related_holons(R::ValueType, vec![old])?;
    fixture.link("Title.PropertyType", R::ValueType, "PreparedEnum")?;
    fixture
        .node("UnusedDefault")?
        .with_property_value("DefinesInstanceTypeKind", false)?
        .with_property_value("IsValueRequired", true)?
        .with_property_value(
            "DefaultValue",
            base_types::BaseValue::EnumValue(base_types::MapEnumValue("Member".into())),
        )?;
    fixture.link("UnusedDefault", R::Extends, "PropertyType.TypeDescriptor")?;
    fixture.link("UnusedDefault", R::ValueType, "PreparedEnum")?;
    Ok(fixture)
}

#[test]
fn enum_and_default_demand_is_subject_local_and_sealed() -> Result<(), HolonError> {
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    use holons_core::descriptors::value_descriptor_subtypes::EnumMembership;
    for saved in [false, true] {
        let fixture = enum_default_fixture()?;
        let fixture = if saved { fixture.saved_snapshot()? } else { fixture };
        let identity = |key: &str| {
            holons_core::DefinitionIdentity::for_reference(&fixture.nodes[key], &fixture.context)
        };
        let mut events = Vec::new();
        let packages = DescriptorPackages::construct_with_observer(
            &fixture.context,
            &[fixture.nodes["Contract"].clone()],
            &[fixture.nodes["UnusedDefault"].clone(), fixture.nodes["PreparedEnum"].clone()],
            &SchemaWorkset { schemas: Vec::new() },
            fixture.diagnoses(),
            &mut |event| events.push(event),
        )?;
        let reads = (fixture.backend_node_reads(), fixture.backend_relationship_reads());
        let probe =
            holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
        let members = packages.enum_members(&identity("PreparedEnum")?)??;
        assert_eq!(
            members.membership("Member"),
            EnumMembership::Member(identity("PreparedVariant")?)
        );
        assert_eq!(members.members()[0].declared_on, identity("PreparedEnum")?);
        assert!(packages.default_declaration(&identity("UnusedDefault")?)?.is_some());
        assert!(packages.requiredness(&fixture.nodes["UnusedDefault"])?.unwrap());
        assert_eq!(
            packages.value_type(&fixture.nodes["UnusedDefault"])?,
            Some(&fixture.nodes["PreparedEnum"])
        );
        assert!(matches!(
            packages.default_declaration(&identity("Title.PropertyType")?),
            Err(HolonError::InvalidState(_))
        ));
        assert!(matches!(
            packages.enum_members(&identity("Contract")?),
            Err(HolonError::InvalidState(_))
        ));
        assert_eq!(packages.preparation_metrics().prepared_enum_definitions, 1);
        assert_eq!(reads, (fixture.backend_node_reads(), fixture.backend_relationship_reads()));
        assert!(probe.finish().0.values().all(|count| *count == 0));
        assert!(events.iter().any(|event| matches!(event, crate::AssessmentEvent::PreparationEdge { role: "EnumDefinition", edge, requests: 1, members: 1 } if edge == "Variants")));

        // Supporting saved declarations are inputs, not independent validation subjects.
        let supporting = DescriptorPackages::construct(
            &fixture.context,
            &[fixture.nodes["Contract"].clone()],
            &[],
            &SchemaWorkset { schemas: Vec::new() },
        )?;
        assert!(matches!(
            supporting.default_declaration(&identity("UnusedDefault")?),
            Err(HolonError::InvalidState(_))
        ));
        assert!(supporting.enum_members(&identity("PreparedEnum")?).is_ok());
    }
    Ok(())
}

#[test]
fn enum_definition_alone_demands_variants_and_local_default_omissions_are_retained(
) -> Result<(), HolonError> {
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    let mut fixture = enum_default_fixture()?;
    let enum_only = DescriptorPackages::construct(
        &fixture.context,
        &[],
        &[fixture.nodes["PreparedEnum"].clone()],
        &SchemaWorkset { schemas: Vec::new() },
    )?;
    assert_eq!(enum_only.preparation_metrics().prepared_enum_definitions, 1);
    assert!(matches!(
        enum_only.value_type(&fixture.nodes["UnusedDefault"]),
        Err(HolonError::InvalidState(_))
    ));
    fixture.node("UnusedDefault")?.remove_property_value("DefaultValue")?;
    fixture
        .node("PropertyType.TypeDescriptor")?
        .with_property_value("DefaultValue", "inherited")?;
    let packages = DescriptorPackages::construct(
        &fixture.context,
        &[],
        &[fixture.nodes["PreparedEnum"].clone(), fixture.nodes["UnusedDefault"].clone()],
        &SchemaWorkset { schemas: Vec::new() },
    )?;
    assert_eq!(packages.preparation_metrics().prepared_enum_definitions, 1);
    let property = holons_core::DefinitionIdentity::for_reference(
        &fixture.nodes["UnusedDefault"],
        &fixture.context,
    )?;
    assert!(packages.default_declaration(&property)?.is_none());
    Ok(())
}

#[test]
fn enum_default_commit_consumes_preparation_without_reads_or_derivation() -> Result<(), HolonError>
{
    for saved in [false, true] {
        let mut fixture = enum_default_fixture()?;
        prepare_meta_contract(&mut fixture)?;
        fixture.link("PreparedEnum", R::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
        fixture.link("UnusedDefault", R::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
        let fixture = if saved { fixture.saved_snapshot()? } else { fixture };
        let mut subject = fixture.staged_subject("enum-consumer")?;
        subject.with_property_value(
            "Title",
            base_types::BaseValue::EnumValue(base_types::MapEnumValue("Member".into())),
        )?;
        let mut candidates = vec![subject];
        if !saved {
            for key in ["PreparedEnum", "UnusedDefault"] {
                let HolonReference::Staged(candidate) = fixture.nodes[key].clone() else {
                    unreachable!()
                };
                candidates.push(candidate);
            }
        }
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

#[test]
fn property_collision_evidence_retains_all_distinct_claimants_and_owners() -> Result<(), HolonError>
{
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    let mut fixture = readiness_fixture()?;
    for key in ["OtherTitle", "ThirdTitle"] {
        fixture
            .node(key)?
            .with_property_value("TypeName", "Title")?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        fixture.link(key, R::Extends, "PropertyType.TypeDescriptor")?;
        fixture.link(key, R::ValueType, "StringValueType.ValueType")?;
        fixture.link("Contract", R::InstanceProperties, key)?;
    }
    // A repeated identity is not another semantic-name claimant.
    fixture.link("Contract", R::InstanceProperties, "OtherTitle")?;
    let fixture = fixture.saved_snapshot()?;
    let packages = DescriptorPackages::construct(
        &fixture.context,
        &[fixture.nodes["Contract"].clone()],
        &[],
        &SchemaWorkset { schemas: Vec::new() },
    )?;
    let reads = (fixture.backend_node_reads(), fixture.backend_relationship_reads());
    let probe = holons_core::core_shared_objects::read_observations::DerivationProbe::start(true);
    let mut products = DescriptorRuleProducts::new(std::sync::Arc::clone(&packages.diagnoses));
    let interpretation = products.prepare_effective_schema(
        packages.effective_schema(&fixture.nodes["Contract"])?,
        &holons_core::DescriptorKindSemantics::new(&fixture.context),
    )?;
    assert!(!interpretation.vocabulary_usable);
    assert_eq!(interpretation.property_name_collisions.len(), 1);
    let owner = holons_core::DefinitionIdentity::for_reference(
        &fixture.nodes["Contract"],
        &fixture.context,
    )?;
    let expected = ["Title.PropertyType", "OtherTitle", "ThirdTitle"]
        .into_iter()
        .map(|key| {
            Ok((
                holons_core::DefinitionIdentity::for_reference(
                    &fixture.nodes[key],
                    &fixture.context,
                )?,
                owner.clone(),
            ))
        })
        .collect::<Result<Vec<_>, HolonError>>()?;
    assert_eq!(
        interpretation.property_name_collisions["Title"]
            .iter()
            .map(|claim| (claim.member.clone(), claim.declared_on.clone()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(reads, (fixture.backend_node_reads(), fixture.backend_relationship_reads()));
    assert!(probe.finish().0.values().all(|count| *count == 0));
    Ok(())
}

#[test]
fn staged_successors_do_not_redirect_saved_enum_or_default_contracts() -> Result<(), HolonError> {
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    use base_types::ToBaseValue;
    use holons_core::core_shared_objects::WritableRelationship;
    use holons_core::descriptors::value_descriptor_subtypes::EnumMembership;
    use type_names::ToPropertyName;
    let fixture = enum_default_fixture()?.saved_snapshot()?;
    let variant = fixture.stage_update_of_with("PreparedVariant", |model| {
        model.properties.insert("TypeName".to_property_name(), "Changed".to_base_value());
        Ok(())
    })?;
    let successor = fixture.stage_update_of_with("PreparedEnum", |model| {
        model.properties.insert("TypeName".to_property_name(), "ChangedEnum".to_base_value());
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        relationships.map.remove(&R::Variants.as_relationship_name());
        relationships.add_related_holons_with_keys(
            R::Variants.as_relationship_name(),
            vec![(variant.clone().into(), None)],
        )?;
        model.relationships = Some(relationships.into());
        Ok(())
    })?;
    let property = fixture.stage_update_of_with("UnusedDefault", |model| {
        model
            .properties
            .insert("DefaultValue".to_property_name(), "ChangedDefault".to_base_value());
        Ok(())
    })?;
    let successor: HolonReference = successor.into();
    let property: HolonReference = property.into();
    let packages = DescriptorPackages::construct(
        &fixture.context,
        &[fixture.nodes["Contract"].clone()],
        &[successor.clone(), property.clone()],
        &SchemaWorkset { schemas: Vec::new() },
    )?;
    let identity = |reference: &HolonReference| {
        holons_core::DefinitionIdentity::for_reference(reference, &fixture.context)
    };
    let original = packages.enum_members(&identity(&fixture.nodes["PreparedEnum"])?)??;
    assert_eq!(
        original.membership("Member"),
        EnumMembership::Member(identity(&fixture.nodes["PreparedVariant"])?)
    );
    assert_eq!(original.membership("Changed"), EnumMembership::Undeclared);
    let changed = packages.enum_members(&identity(&successor)?)??;
    assert_eq!(changed.membership("Changed"), EnumMembership::Member(identity(&variant.into())?));
    assert_eq!(changed.membership("Member"), EnumMembership::Undeclared);
    assert_eq!(
        packages.default_declaration(&identity(&property)?)?,
        Some(&"ChangedDefault".to_base_value())
    );
    assert!(matches!(
        packages.default_declaration(&identity(&fixture.nodes["UnusedDefault"])?),
        Err(HolonError::InvalidState(_))
    ));
    assert_eq!(packages.value_type(&property)?, Some(&fixture.nodes["PreparedEnum"]));
    Ok(())
}

#[test]
fn malformed_variant_fields_are_sealed_defects() -> Result<(), HolonError> {
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    for field in ["TypeName", "IsAbstractType"] {
        let mut fixture = enum_default_fixture()?;
        fixture.node("PreparedVariant")?.remove_property_value(field)?;
        let packages = DescriptorPackages::construct(
            &fixture.context,
            &[],
            &[fixture.nodes["PreparedEnum"].clone()],
            &SchemaWorkset { schemas: Vec::new() },
        )?;
        let identity = holons_core::DefinitionIdentity::for_reference(
            &fixture.nodes["PreparedEnum"],
            &fixture.context,
        )?;
        assert!(
            matches!(packages.enum_members(&identity)?, Err(HolonError::EmptyField(name)) if name == field)
        );
        assert_eq!(packages.preparation_metrics().prepared_enum_definitions, 1);
    }
    Ok(())
}

#[test]
fn enum_member_acquisition_failure_is_not_a_declaration_defect() -> Result<(), HolonError> {
    use crate::assessment::{descriptor_package::DescriptorPackages, schema_view::SchemaWorkset};
    // Even a backend error resembling a field defect must remain operational.
    let error = HolonError::EmptyField("backend acquisition failed".into());
    let fixture = enum_default_fixture()?
        .saved_snapshot_with_read_errors(&[("PreparedVariant", error.clone())])?;
    let result = DescriptorPackages::construct(
        &fixture.context,
        &[],
        &[fixture.nodes["PreparedEnum"].clone()],
        &SchemaWorkset { schemas: Vec::new() },
    );
    assert_eq!(result.err(), Some(error));
    Ok(())
}
