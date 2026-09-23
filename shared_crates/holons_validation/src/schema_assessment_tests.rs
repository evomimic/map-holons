use super::*;
use crate::{
    assessment_support::path,
    orchestration::PreparedAssessment,
    schema_rules::{cross_schema_references, dependency_cycles, SchemaRuleProducts},
    schema_view::{OwnedCandidate, SchemaWorkset},
};
use holons_core::{
    HolonCollection, HolonCollectionApi, ProspectiveDescriptorReader, ProspectiveIdentity,
    ReadableHolon, SchemaOwnershipKind, StagedReference,
};
use std::sync::{Arc, RwLock};
use type_names::ToRelationshipName;

fn schema_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = Fixture::empty()?;
    for key in [
        "TypeDescriptor",
        "Schema.HolonType",
        "A",
        "B",
        "C",
        "Member",
        "OtherMember",
        "OwnedRule",
        "Foreign",
    ] {
        fixture.node(key)?;
    }
    fixture.link("Schema.HolonType", CoreRelationshipTypeName::Extends, "TypeDescriptor")?;
    Ok(fixture)
}
fn describe_schemas(fixture: &mut Fixture) -> Result<(), HolonError> {
    for schema in ["A", "B", "C"] {
        fixture.link(schema, CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    }
    Ok(())
}
fn staged(reference: &HolonReference) -> StagedReference {
    let HolonReference::Staged(reference) = reference else { panic!("staged fixture reference") };
    reference.clone()
}
fn owned(fixture: &Fixture, key: &str, kind: SchemaOwnershipKind) -> OwnedCandidate {
    OwnedCandidate { subject: fixture.nodes[key].clone(), kind }
}
fn replace_edge(
    fixture: &Fixture,
    key: &str,
    edge: CoreRelationshipTypeName,
    target: &str,
) -> Result<StagedReference, HolonError> {
    fixture.replacement_with(key, |model| {
        let mut members = HolonCollection::new_transient();
        members.add_references(vec![fixture.nodes[target].clone()])?;
        model
            .relationships
            .as_mut()
            .unwrap()
            .insert(edge.to_relationship_name(), Arc::new(RwLock::new(members)));
        Ok(())
    })
}

#[test]
fn descriptor_only_staging_schedules_one_schema_and_keeps_memberships_separate(
) -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    for key in ["Member", "OtherMember"] {
        fixture.link(key, CoreRelationshipTypeName::ComponentOf, "A")?;
    }
    fixture.link("OwnedRule", CoreRelationshipTypeName::RuleOf, "A")?;
    describe_schemas(&mut fixture)?;
    let candidates = [
        staged(&fixture.nodes["Member"]),
        staged(&fixture.nodes["OtherMember"]),
        staged(&fixture.nodes["OwnedRule"]),
    ];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "OtherMember", SchemaOwnershipKind::Component),
            owned(&fixture, "OwnedRule", SchemaOwnershipKind::Rule),
        ],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 1);
    assert_eq!(workset.schemas[0].components.len(), 2);
    assert_eq!(workset.schemas[0].rules.len(), 1);
    assert!(collector.into_report().is_accepted());
    assert!(!candidates
        .iter()
        .any(|candidate| candidate.temporary_id() == staged(&fixture.nodes["A"]).temporary_id()));
    Ok(())
}

#[test]
fn replacements_move_both_membership_namespaces_and_assess_both_owners() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    for key in ["Member", "OtherMember"] {
        fixture.link(key, CoreRelationshipTypeName::ComponentOf, "A")?;
        fixture.link("A", CoreRelationshipTypeName::Components, key)?;
    }
    fixture.link("OwnedRule", CoreRelationshipTypeName::RuleOf, "A")?;
    fixture.link("A", CoreRelationshipTypeName::Rules, "OwnedRule")?;
    describe_schemas(&mut fixture)?;
    let fixture = fixture.saved_snapshot()?;
    let member = replace_edge(&fixture, "Member", CoreRelationshipTypeName::ComponentOf, "B")?;
    let rule = replace_edge(&fixture, "OwnedRule", CoreRelationshipTypeName::RuleOf, "B")?;
    let candidates = [member.clone(), rule.clone()];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            OwnedCandidate { subject: member.clone().into(), kind: SchemaOwnershipKind::Component },
            OwnedCandidate { subject: rule.clone().into(), kind: SchemaOwnershipKind::Rule },
        ],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 2);
    let a = workset
        .schemas
        .iter()
        .find(|view| holons_core::same_definition(&view.schema, &fixture.nodes["A"]))
        .unwrap();
    let b = workset
        .schemas
        .iter()
        .find(|view| holons_core::same_definition(&view.schema, &fixture.nodes["B"]))
        .unwrap();
    assert_eq!(a.components.len(), 1);
    assert!(holons_core::same_definition(&a.components[0], &fixture.nodes["OtherMember"]));
    assert!(a.rules.is_empty());
    assert_eq!(b.components.len(), 1);
    assert_eq!(b.rules.len(), 1);
    assert!(holons_core::same_definition(&b.components[0], &member.into()));
    assert!(holons_core::same_definition(&b.rules[0], &rule.into()));
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn staged_schema_dependency_only_change_is_assessed_and_reaches_cycle() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("A", CoreRelationshipTypeName::DependsOn, "B")?;
    describe_schemas(&mut fixture)?;
    let fixture = fixture.saved_snapshot()?;
    let update = replace_edge(&fixture, "B", CoreRelationshipTypeName::DependsOn, "A")?;
    let reader = ProspectiveDescriptorReader::new(&fixture.context, std::slice::from_ref(&update))?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[update.clone().into()],
        &[],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 1);
    let cycles = dependency_cycles(&fixture.context, &reader, &workset.schemas, &mut collector)?;
    assert!(
        cycles[&ProspectiveIdentity::for_reference(&update.into(), &fixture.context)?].is_some()
    );
    Ok(())
}

#[test]
fn direct_dependencies_are_required_for_components_and_owned_rules() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("A", CoreRelationshipTypeName::DependsOn, "B")?;
    fixture.link("B", CoreRelationshipTypeName::DependsOn, "C")?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("OwnedRule", CoreRelationshipTypeName::RuleOf, "A")?;
    fixture.link("Foreign", CoreRelationshipTypeName::ComponentOf, "C")?;
    for source in ["Member", "OwnedRule"] {
        fixture.link(source, CoreRelationshipTypeName::ValueType, "Foreign")?;
    }
    describe_schemas(&mut fixture)?;
    let candidates = [staged(&fixture.nodes["Member"]), staged(&fixture.nodes["OwnedRule"])];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "OwnedRule", SchemaOwnershipKind::Rule),
        ],
        &mut collector,
    )?;
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(
        &fixture.context,
        &reader,
        &workset.schemas[0],
        &workset,
        &mut products,
        &mut collector,
    )?;
    let rule = CoreValidationRuleName::CrossSchemaDependenciesDeclared;
    fixture.node(rule.as_str())?;
    let binding = ResolvedValidationBinding {
        rule: fixture.nodes[rule.as_str()].clone(),
        declaring_descriptor: holons_core::HolonDescriptor::from_holon(
            fixture.nodes["Schema.HolonType"].clone(),
        ),
    };
    StaticRuleRegistry::lookup(&ValidationRuleKey(rule.as_str().into())).unwrap()(
        ValidationInvocation::Schema {
            binding: &binding,
            path: &path(&fixture.nodes["A"]),
            products: &products,
        },
        &mut collector,
    )?;
    let report = collector.into_report();
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations.iter().all(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-SCHEMA-002")));
    assert!(report.violations[0].message.contains("2 authored reference(s)"));
    assert!(report.violations[0].message.contains(&fixture.nodes["C"].reference_id_string()));
    Ok(())
}

#[test]
fn schema_references_ignore_space_instances_and_schema_targets() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.node("Space")?;
    fixture.link("Space", CoreRelationshipTypeName::OwnedBy, "Space")?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("Member", CoreRelationshipTypeName::OwnedBy, "Space")?;
    fixture.link("Member", CoreRelationshipTypeName::ValueType, "B")?;
    describe_schemas(&mut fixture)?;
    let candidate = staged(&fixture.nodes["Member"]);
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &[candidate])?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[owned(&fixture, "Member", SchemaOwnershipKind::Component)],
        &mut collector,
    )?;
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(
        &fixture.context,
        &reader,
        &workset.schemas[0],
        &workset,
        &mut products,
        &mut collector,
    )?;
    assert!(!products.has_findings());
    assert!(collector.into_report().is_accepted());
    assert_eq!(
        fixture.nodes["Space"]
            .related_holons(CoreRelationshipTypeName::OwnedBy)?
            .read()
            .unwrap()
            .get_count()
            .0,
        1,
        "the space self-reference remains authored but is outside the Schema component workset"
    );
    Ok(())
}

#[test]
fn same_schema_component_reference_needs_no_dependency() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    for key in ["Member", "OtherMember"] {
        fixture.link(key, CoreRelationshipTypeName::ComponentOf, "A")?;
    }
    fixture.link("Member", CoreRelationshipTypeName::ValueType, "OtherMember")?;
    describe_schemas(&mut fixture)?;
    let candidates = [staged(&fixture.nodes["Member"]), staged(&fixture.nodes["OtherMember"])];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "OtherMember", SchemaOwnershipKind::Component),
        ],
        &mut collector,
    )?;
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(
        &fixture.context,
        &reader,
        &workset.schemas[0],
        &workset,
        &mut products,
        &mut collector,
    )?;
    assert!(!products.has_findings());
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn direct_dependency_covers_cross_schema_component_reference() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("A", CoreRelationshipTypeName::DependsOn, "B")?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("Foreign", CoreRelationshipTypeName::ComponentOf, "B")?;
    fixture.link("Member", CoreRelationshipTypeName::ValueType, "Foreign")?;
    describe_schemas(&mut fixture)?;
    let candidates = [staged(&fixture.nodes["Member"]), staged(&fixture.nodes["Foreign"])];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "Foreign", SchemaOwnershipKind::Component),
        ],
        &mut collector,
    )?;
    let a = workset
        .schemas
        .iter()
        .find(|view| holons_core::same_definition(&view.schema, &fixture.nodes["A"]))
        .unwrap();
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(&fixture.context, &reader, a, &workset, &mut products, &mut collector)?;
    assert!(!products.has_findings());
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn missing_target_ownership_keeps_its_own_finding() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("Member", CoreRelationshipTypeName::ValueType, "Foreign")?;
    describe_schemas(&mut fixture)?;
    let candidates = [staged(&fixture.nodes["Member"]), staged(&fixture.nodes["Foreign"])];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "Foreign", SchemaOwnershipKind::Component),
        ],
        &mut collector,
    )?;
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(
        &fixture.context,
        &reader,
        &workset.schemas[0],
        &workset,
        &mut products,
        &mut collector,
    )?;
    assert!(!products.has_findings());
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 1);
    assert!(
        matches!(&report.violations[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "SchemaOwnership")
    );
    assert_eq!(report.violations[0].subject, path(&fixture.nodes["Foreign"]));
    Ok(())
}

#[test]
fn ambiguous_target_ownership_blocks_once_per_target() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("OwnedRule", CoreRelationshipTypeName::RuleOf, "A")?;
    for owner in ["B", "C"] {
        fixture.link("Foreign", CoreRelationshipTypeName::ComponentOf, owner)?;
    }
    for source in ["Member", "OwnedRule"] {
        fixture.link(source, CoreRelationshipTypeName::ValueType, "Foreign")?;
    }
    describe_schemas(&mut fixture)?;
    let candidates = [
        staged(&fixture.nodes["Member"]),
        staged(&fixture.nodes["OwnedRule"]),
        staged(&fixture.nodes["Foreign"]),
    ];
    let reader = ProspectiveDescriptorReader::new(&fixture.context, &candidates)?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "OwnedRule", SchemaOwnershipKind::Rule),
            owned(&fixture, "Foreign", SchemaOwnershipKind::Component),
        ],
        &mut collector,
    )?;
    let a = workset
        .schemas
        .iter()
        .find(|view| holons_core::same_definition(&view.schema, &fixture.nodes["A"]))
        .unwrap();
    let mut products = SchemaRuleProducts::default();
    cross_schema_references(&fixture.context, &reader, a, &workset, &mut products, &mut collector)?;
    assert!(!products.has_findings());
    let report = collector.into_report();
    assert_eq!(report.violation_count(), 2);
    assert!(report.violations.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "SchemaOwnership")));
    assert!(report.violations.iter().any(|finding| {
        finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency
            && finding.message.contains("2 authored reference(s)")
    }));
    Ok(())
}

#[test]
fn missing_multiple_and_invalid_owners_are_findings_in_both_namespaces() -> Result<(), HolonError> {
    for kind in [SchemaOwnershipKind::Component, SchemaOwnershipKind::Rule] {
        for owners in [vec![], vec!["A", "B"], vec!["Foreign"]] {
            let mut fixture = schema_fixture()?;
            for owner in &owners {
                fixture.link("Member", crate::schema_view::ownership_edge(kind), owner)?;
            }
            describe_schemas(&mut fixture)?;
            let reader = ProspectiveDescriptorReader::new(
                &fixture.context,
                &[staged(&fixture.nodes["Member"])],
            )?;
            let mut collector = ValidationCollector::default();
            let workset = SchemaWorkset::prepare(
                &fixture.context,
                &reader,
                &fixture.nodes["Schema.HolonType"],
                &[],
                &[owned(&fixture, "Member", kind)],
                &mut collector,
            )?;
            assert_eq!(
                workset.ownership.len(),
                1,
                "invalid ownership must not drop the staged commitment"
            );
            assert_eq!(workset.schemas.len(), owners.len());
            assert_eq!(collector.into_report().violation_count(), 1);
        }
    }
    Ok(())
}

#[test]
fn schema_scoped_findings_install_on_their_subject_in_either_candidate_order(
) -> Result<(), HolonError> {
    let fixture = Fixture::new()?;
    let descriptor = fixture.staged_subject("descriptor")?;
    let constraint = fixture.staged_subject("constraint")?;
    let mut collector = ValidationCollector::default();
    crate::handlers::finding(
        &mut collector,
        CommitValidationViolationKind::RuleViolation { code: "DS-CONSTRAINT-003".into() },
        None,
        &path(&constraint.clone().into()),
        None,
        "Constraint configuration is invalid.".into(),
    );
    let finding = collector.into_report().violations.remove(0);
    for candidates in
        [[descriptor.clone(), constraint.clone()], [constraint.clone(), descriptor.clone()]]
    {
        let report = PreparedAssessment::from_scope(
            &candidates,
            CommitValidationReport::from_candidate(vec![finding.clone()]),
        )?
        .install_outcomes()?;
        assert!(descriptor.validation_findings()?.is_empty());
        assert_eq!(constraint.validation_findings()?, vec![finding.clone()]);
        assert!(report.unattached_findings()?.is_empty());
    }
    let report = PreparedAssessment::from_scope(
        std::slice::from_ref(&descriptor),
        CommitValidationReport::from_candidate(vec![finding.clone()]),
    )?
    .install_outcomes()?;
    assert_eq!(report.unattached_findings()?, vec![&finding]);
    assert!(descriptor.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn public_gate_rejects_identical_competitors_and_preserves_independent_findings(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("Existing")?;
    fixture.link("Existing", CoreRelationshipTypeName::DescribedBy, "Contract")?;
    let fixture = fixture.saved_snapshot()?;
    let first = fixture.replacement("Existing")?;
    let second = fixture.replacement("Existing")?;
    let independent = fixture.staged_subject("independent")?;
    let report = validate_commit_candidates(
        &fixture.context,
        &[first.clone(), second.clone(), independent.clone()],
    )?;
    assert!(!report.is_accepted());
    for candidate in [&first, &second] {
        assert!(candidate.validation_findings()?.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "CompetingStagedReplacements")));
    }
    assert!(independent.validation_findings()?.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-PROP-001")));
    second.abandon_staged_changes(&fixture.context)?;
    let mut first = first;
    first.with_property_value("Title", "corrected")?;
    // Strip fixture-only metadata that is not part of the ordinary Contract.
    for property in ["TypeName", "IsAbstractType", "IsValueRequired"] {
        first.remove_property_value(property)?;
    }
    assert!(
        validate_commit_candidates(&fixture.context, std::slice::from_ref(&first))?.is_accepted()
    );
    assert!(first.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn operational_scope_failure_preserves_prior_installed_outcomes() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
    let candidate = fixture.staged_subject("prior")?;
    let old = validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
    let foreign = Fixture::new()?;
    let reader =
        ProspectiveDescriptorReader::new(&fixture.context, std::slice::from_ref(&candidate))?;
    let mut collector = ValidationCollector::default();
    assert!(SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Contract"],
        &[foreign.nodes["Contract"].clone()],
        &[],
        &mut collector
    )
    .is_err());
    assert_eq!(candidate.validation_findings()?, old.violations);
    Ok(())
}

/// Complete the roots and member structure consumed by prospective Commit assessment.
fn readiness_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = Fixture::new()?;
    c2_kind_roots(&mut fixture)?;
    for key in [
        "Schema.HolonType",
        "Rule.HolonType",
        "ConstraintType.HolonType",
        "StringLengthConstraint.ConstraintType",
        "BytesLengthConstraint.ConstraintType",
        "NumericRangeConstraint.ConstraintType",
        "ItemCountConstraint.ConstraintType",
        "CardinalityConstraint.ConstraintType",
        "UniqueItemsConstraint.ConstraintType",
        "ValueType.TypeDescriptor",
        "DeclaredRelationshipType.RelationshipType",
    ] {
        fixture.node(key)?;
    }
    fixture.link(
        "DeclaredRelationshipType",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link(
        "DeclaredRelationshipType.RelationshipType",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    fixture.link(
        "ValueType.TypeDescriptor",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    // StringValueType's prior direct TypeDescriptor edge is replaced, never supplemented.
    let type_descriptor = fixture.nodes["TypeDescriptor"].clone();
    fixture
        .nodes
        .get_mut("StringValueType.ValueType")
        .unwrap()
        .remove_related_holons(CoreRelationshipTypeName::Extends, vec![type_descriptor])?;
    fixture.link(
        "StringValueType.ValueType",
        CoreRelationshipTypeName::Extends,
        "ValueType.TypeDescriptor",
    )?;
    for key in [
        "Title.PropertyType",
        "Key.PropertyType",
        "DeclaredRelationshipType",
        "DescribedBy.Relationship",
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value("DefinesInstanceTypeKind", false)?;
    }
    for key in [
        "PropertyType.TypeDescriptor",
        "ValueType.TypeDescriptor",
        "DeclaredRelationshipType.RelationshipType",
        "StringValueType.ValueType",
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value("DefinesInstanceTypeKind", true)?
            .with_property_value("IsAbstractType", true)?;
    }
    fixture.link(
        "DescribedBy.Relationship",
        CoreRelationshipTypeName::SourceType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "DescribedBy.Relationship",
        CoreRelationshipTypeName::TargetType,
        "HolonType.TypeDescriptor",
    )?;
    for rule in [
        CoreValidationRuleName::AtMostOneDirectParent,
        CoreValidationRuleName::AcyclicExtendsLineage,
        CoreValidationRuleName::ExtendsLineageTerminatesAtTypeDescriptor,
        CoreValidationRuleName::UniqueTypeDescriptorRoot,
        CoreValidationRuleName::LocalInstanceKindAnchorDesignation,
        CoreValidationRuleName::InstanceKindAnchorsAreAbstract,
        CoreValidationRuleName::TypeDescriptorRootKindException,
        CoreValidationRuleName::DescribingCategoryCompatibility,
        CoreValidationRuleName::DescriptorMetaTypeCorrespondence,
        CoreValidationRuleName::NoInheritedMemberRedeclaration,
        CoreValidationRuleName::UniqueSemanticMemberNames,
        CoreValidationRuleName::WellFormedEffectiveMemberDefinitions,
        CoreValidationRuleName::ContractMemberKindCompatibility,
        CoreValidationRuleName::InheritedValueConstraintNonRelaxation,
        CoreValidationRuleName::SchemaDependenciesAcyclic,
        CoreValidationRuleName::CrossSchemaDependenciesDeclared,
    ] {
        fixture.node(rule.as_str())?;
        fixture.link(
            rule.as_str(),
            CoreRelationshipTypeName::DescribedBy,
            "HolonValidationRule.HolonType",
        )?;
    }
    Ok(fixture)
}

#[test]
fn commit_readiness_accepts_valid_subject_and_reassesses_a_corrected_contract(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("instance")?;
    subject.with_property_value("Title", "ready")?;
    assert!(crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject)
    )?
    .is_accepted());
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    let report = crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject),
    )?;
    assert!(!report.is_accepted());
    assert!(subject
        .validation_findings()?
        .iter()
        .any(|finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "Title")?;
    assert!(crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject)
    )?
    .is_accepted());
    assert!(subject.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn commit_assessment_operational_failure_preserves_prior_outcomes() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let subject = fixture.staged_subject("previously-rejected")?;
    let old = crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject),
    )?;
    assert!(!old.is_accepted());
    // A reference bound to a foreign transaction is an inability to assess reliably,
    // not a readable descriptor invariant violation.
    let foreign = Fixture::new()?;
    fixture.nodes.get_mut("Contract").unwrap().add_related_holons(
        CoreRelationshipTypeName::InstanceProperties,
        vec![foreign.nodes["Title.PropertyType"].clone()],
    )?;
    assert!(crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject)
    )
    .is_err());
    assert_eq!(subject.validation_findings()?, old.violations);
    Ok(())
}

#[test]
fn unstaged_schema_cycle_reaches_the_carrier_and_blocks_its_staged_component(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    for rule in [
        CoreValidationRuleName::SchemaDependenciesAcyclic,
        CoreValidationRuleName::CrossSchemaDependenciesDeclared,
    ] {
        fixture.link(
            "Schema.HolonType",
            CoreRelationshipTypeName::ValidationBindings,
            rule.as_str(),
        )?;
    }
    for key in ["OwnerA", "OwnerB", "StagedComponent"] {
        fixture.node(key)?;
    }
    fixture.link("OwnerA", CoreRelationshipTypeName::DependsOn, "OwnerB")?;
    fixture.link("OwnerB", CoreRelationshipTypeName::DependsOn, "OwnerA")?;
    for key in ["OwnerA", "OwnerB"] {
        fixture.link(key, CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    }
    fixture.link(
        "StagedComponent",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link("StagedComponent", CoreRelationshipTypeName::ComponentOf, "OwnerA")?;
    fixture
        .nodes
        .get_mut("StagedComponent")
        .unwrap()
        .with_property_value("DefinesInstanceTypeKind", false)?;
    fixture.link(
        "StagedComponent",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let candidate = staged(&fixture.nodes["StagedComponent"]);
    let report = crate::readiness::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&candidate),
    )?;
    let aggregates = report.unattached_findings()?;
    let cycles = aggregates.iter().filter(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-SCHEMA-001")).collect::<Vec<_>>();
    assert_eq!(cycles.len(), 1, "one assessment of the affected owner");
    assert_eq!(cycles[0].subject, path(&fixture.nodes["OwnerA"]));
    assert!(candidate
        .validation_findings()?
        .iter()
        .any(|finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    Ok(())
}

#[test]
fn contested_members_keep_saved_membership_without_selecting_a_competitor() -> Result<(), HolonError>
{
    let mut fixture = schema_fixture()?;
    fixture.link("Member", CoreRelationshipTypeName::ComponentOf, "A")?;
    fixture.link("A", CoreRelationshipTypeName::Components, "Member")?;
    describe_schemas(&mut fixture)?;
    let fixture = fixture.saved_snapshot()?;
    let first = fixture.replacement("Member")?;
    let second = fixture.replacement("Member")?;
    let reader =
        ProspectiveDescriptorReader::new(&fixture.context, &[first.clone(), second.clone()])?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &reader,
        &fixture.nodes["Schema.HolonType"],
        &[],
        &[
            OwnedCandidate { subject: first.into(), kind: SchemaOwnershipKind::Component },
            OwnedCandidate { subject: second.into(), kind: SchemaOwnershipKind::Component },
        ],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 1);
    assert_eq!(workset.schemas[0].components.len(), 1);
    assert!(matches!(&workset.schemas[0].components[0], HolonReference::Smart(_)));
    assert!(collector
        .into_report()
        .violations
        .iter()
        .all(|finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    Ok(())
}

#[test]
fn contested_shared_anchors_use_transaction_carriers_in_either_candidate_order(
) -> Result<(), HolonError> {
    for anchor in
        [CoreValidationRuleName::NoUndescribedProperties.as_str(), "MetaTypeDescriptor.HolonType"]
    {
        let fixture = readiness_fixture()?.saved_snapshot()?;
        let first = fixture.replacement(anchor)?;
        let second = fixture.replacement(anchor)?;
        let independent = fixture.staged_subject("independent")?;
        for assess in [validate_commit_candidates] {
            let mut previous = None;
            for candidates in [
                [independent.clone(), first.clone(), second.clone()],
                [second.clone(), first.clone(), independent.clone()],
            ] {
                let report = assess(&fixture.context, &candidates)?;
                let transaction_findings = report
                    .unattached_findings()?
                    .into_iter()
                    .filter(|finding| finding.subject == ValidationSubjectPath::Transaction)
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(transaction_findings.len(), 1);
                assert_eq!(
                    transaction_findings[0].kind,
                    CommitValidationViolationKind::UnresolvedLocalDependency
                );
                assert!(transaction_findings[0]
                    .message
                    .contains(&fixture.nodes[anchor].holon_id()?.to_string()));
                for candidate in &candidates {
                    assert!(candidate
                        .validation_findings()?
                        .iter()
                        .all(|finding| crate::orchestration::subject_identity(&finding.subject)
                            == Some(candidate.reference_id_string().as_str())));
                }
                let outcomes = [
                    independent.validation_findings()?,
                    first.validation_findings()?,
                    second.validation_findings()?,
                ];
                if let Some((prior_transaction, prior_outcomes)) = &previous {
                    assert_eq!(&transaction_findings, prior_transaction);
                    assert_eq!(&outcomes, prior_outcomes);
                }
                previous = Some((transaction_findings, outcomes));
            }
        }
    }
    Ok(())
}

#[test]
fn scoped_installation_uses_the_same_distinct_candidate_check_as_commit() -> Result<(), HolonError>
{
    let fixture = Fixture::new()?;
    let candidate = fixture.staged_subject("repeated")?;
    let candidates = [candidate.clone(), candidate.clone()];
    let expected = crate::orchestration::check_candidates(&candidates).unwrap_err();
    let Err(actual) =
        PreparedAssessment::from_scope(&candidates, CommitValidationReport::default())
    else {
        panic!("repeated candidates must reject before installation")
    };
    assert_eq!(actual, expected);
    assert_eq!(candidate.validation_state()?, ValidationState::ValidationRequired);
    Ok(())
}
