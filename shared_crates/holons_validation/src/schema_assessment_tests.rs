use super::*;
use crate::{
    assessment_support::path,
    outcomes::PreparedAssessment,
    schema_rules::dependency_cycles,
    schema_view::{OwnedCandidate, SchemaWorkset},
};
use holons_core::{
    DefinitionIdentity, HolonCollection, HolonCollectionApi, ReadableHolon, SchemaOwnershipKind,
    StagedReference,
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
    fixture.stage_update_of_with(key, |model| {
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
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &[],
        &[
            owned(&fixture, "Member", SchemaOwnershipKind::Component),
            owned(&fixture, "OtherMember", SchemaOwnershipKind::Component),
            owned(&fixture, "OwnedRule", SchemaOwnershipKind::Rule),
        ],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 1);
    assert_eq!(workset.schemas[0].staged_components.len(), 2);
    assert_eq!(workset.schemas[0].staged_rules.len(), 1);
    assert!(collector.into_report().is_accepted());
    assert!(!candidates
        .iter()
        .any(|candidate| candidate.temporary_id() == staged(&fixture.nodes["A"]).temporary_id()));
    Ok(())
}

#[test]
fn successors_establish_new_memberships_without_scheduling_previous_owners(
) -> Result<(), HolonError> {
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
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &[],
        &[
            OwnedCandidate { subject: member.clone().into(), kind: SchemaOwnershipKind::Component },
            OwnedCandidate { subject: rule.clone().into(), kind: SchemaOwnershipKind::Rule },
        ],
        &mut collector,
    )?;
    assert_eq!(workset.schemas.len(), 1);
    let b = workset
        .schemas
        .iter()
        .find(|view| {
            holons_core::same_definition(&view.schema, &fixture.nodes["B"])
                .expect("readable fixture identity")
        })
        .unwrap();
    assert_eq!(b.staged_components.len(), 1);
    assert_eq!(b.staged_rules.len(), 1);
    assert!(holons_core::same_definition(&b.staged_components[0], &member.into())?);
    assert!(holons_core::same_definition(&b.staged_rules[0], &rule.into())?);
    assert!(collector.into_report().is_accepted());
    Ok(())
}

#[test]
fn staged_schema_dependency_follows_exact_saved_targets_without_substitution(
) -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("A", CoreRelationshipTypeName::DependsOn, "B")?;
    describe_schemas(&mut fixture)?;
    let fixture = fixture.saved_snapshot()?;
    let update = replace_edge(&fixture, "B", CoreRelationshipTypeName::DependsOn, "A")?;
    let mut collector = ValidationCollector::default();
    let workset =
        SchemaWorkset::prepare(&fixture.context, &[update.clone().into()], &[], &mut collector)?;
    assert_eq!(workset.schemas.len(), 1);
    let cycles = dependency_cycles(&fixture.context, &workset.schemas)?;
    assert!(cycles[&DefinitionIdentity::for_reference(&update.into(), &fixture.context)?].is_none());
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
            let mut collector = ValidationCollector::default();
            let workset = SchemaWorkset::prepare(
                &fixture.context,
                &[],
                &[owned(&fixture, "Member", kind)],
                &mut collector,
            )?;
            for view in &workset.schemas {
                let members = match kind {
                    SchemaOwnershipKind::Component => &view.staged_components,
                    SchemaOwnershipKind::Rule => &view.staged_rules,
                };
                assert_eq!(members.len(), 1, "invalid ownership must not drop the staged member");
                assert!(holons_core::same_definition(&members[0], &fixture.nodes["Member"])?);
            }
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
fn operational_scope_failure_preserves_prior_installed_outcomes() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
    let candidate = fixture.staged_subject("prior")?;
    let old = validate_commit_candidates(&fixture.context, std::slice::from_ref(&candidate))?;
    let foreign = Fixture::new()?;
    let mut collector = ValidationCollector::default();
    assert!(SchemaWorkset::prepare(
        &fixture.context,
        &[foreign.nodes["Contract"].clone()],
        &[],
        &mut collector
    )
    .is_err());
    assert_eq!(candidate.validation_findings()?, old.violations);
    Ok(())
}

#[test]
fn staged_descriptor_with_unnamed_effective_member_reports_contract_finding(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::ValidationBindings,
        CoreValidationRuleName::WellFormedEffectiveMemberDefinitions.as_str(),
    )?;
    fixture.link(
        "Contract",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    let descriptor = staged(&fixture.nodes["Contract"]);

    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&descriptor))?;
    assert!(!report.is_accepted());
    assert!(descriptor.validation_findings()?.iter().any(|finding| {
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONTRACT-003")
            && finding.rule_key.as_deref()
                == Some(CoreValidationRuleName::WellFormedEffectiveMemberDefinitions.as_str())
            && finding.subject == path(&fixture.nodes["Contract"])
            && finding.message.contains(&fixture.nodes["Title.PropertyType"].reference_id_string())
    }));
    Ok(())
}

#[test]
fn ordinary_holon_lineage_defect_is_reported_before_descriptor_classification(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("UnrootedParent")?;
    let transient = fixture.context.mutation().new_holon(Some("ordinary-parented".into()))?;
    let mut subject = fixture.context.mutation().stage_new_holon(transient)?;
    subject.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![fixture.nodes["UnrootedParent"].clone()],
    )?;
    subject.with_descriptor(fixture.nodes["Contract"].clone())?;
    subject.with_property_value("Title", "present")?;
    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!report.is_accepted());
    assert!(subject.validation_findings()?.iter().any(|finding| {
        matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-STRUCT-004")
            && finding.subject == path(&subject.clone().into())
    }));
    Ok(())
}

#[test]
fn unique_describer_with_malformed_lineage_blocks_its_ordinary_subject() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("SecondParent")?;
    let mut subject = fixture.staged_subject("invalid-governor")?;
    subject.with_property_value("Title", "present")?;
    fixture.link("Contract", CoreRelationshipTypeName::Extends, "SecondParent")?;
    let report = validate_commit_candidates(&fixture.context, std::slice::from_ref(&subject))?;
    assert!(!report.is_accepted());
    assert!(subject.validation_findings()?.iter().any(|finding| {
        finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency
            && finding.message.contains(&fixture.nodes["Contract"].reference_id_string())
    }));
    Ok(())
}

#[test]
fn commit_readiness_accepts_valid_subject_and_reassesses_a_corrected_contract(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("instance")?;
    subject.with_property_value("Title", "ready")?;
    assert!(crate::commit_assessment::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject)
    )?
    .is_accepted());
    fixture.nodes.get_mut("Title.PropertyType").unwrap().remove_property_value("TypeName")?;
    let report = crate::commit_assessment::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject),
    )?;
    assert!(!report.is_accepted());
    assert!(subject
        .validation_findings()?
        .iter()
        .any(|finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency));
    assert!(subject.validation_findings()?.iter().all(|finding| {
        !matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-CONTRACT-003")
    }), "an unstaged malformed descriptor blocks its dependent subject");
    fixture
        .nodes
        .get_mut("Title.PropertyType")
        .unwrap()
        .with_property_value("TypeName", "Title")?;
    assert!(crate::commit_assessment::validate_commit_candidates(
        &fixture.context,
        std::slice::from_ref(&subject)
    )?
    .is_accepted());
    assert!(subject.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn report_only_commit_assessment_preserves_outcomes_and_matches_validation(
) -> Result<(), HolonError> {
    let fixture = readiness_fixture()?;
    let mut subject = fixture.staged_subject("report-only-instance")?;
    let candidates = std::slice::from_ref(&subject);
    let initial = assess_commit_candidates(&fixture.context, candidates)?;
    assert!(!initial.report.is_accepted());
    assert_eq!(subject.validation_state()?, ValidationState::ValidationRequired);
    assert!(subject.validation_findings()?.is_empty());
    let rejected = validate_commit_candidates(&fixture.context, candidates)?;
    assert_eq!(initial.report, rejected);

    subject.with_property_value("Title", "ready")?;
    // Content mutation requires revalidation. Establish a prior outcome to prove assessment
    // preserves both the state and findings even when the newly assessed report differs.
    subject.replace_validation_outcome(ValidationState::Invalid, rejected.violations.clone())?;
    let title = subject.property_value("Title")?;
    let candidates = std::slice::from_ref(&subject);
    let accepted = assess_commit_candidates(&fixture.context, candidates)?;
    assert!(accepted.report.is_accepted());
    assert!(accepted
        .observations
        .discovered_rule_keys
        .contains(CoreValidationRuleName::NoUndescribedProperties.as_str()));
    assert!(accepted
        .observations
        .dispatched_rule_keys
        .contains(CoreValidationRuleName::NoUndescribedProperties.as_str()));
    assert_eq!(subject.validation_state()?, ValidationState::Invalid);
    assert_eq!(subject.validation_findings()?, rejected.violations);
    assert_eq!(subject.property_value("Title")?, title);
    assert_eq!(accepted.report, validate_commit_candidates(&fixture.context, candidates)?);
    assert_eq!(subject.validation_state()?, ValidationState::Validated);
    assert!(subject.validation_findings()?.is_empty());
    Ok(())
}

#[test]
fn commit_assessment_operational_failure_preserves_prior_outcomes() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let subject = fixture.staged_subject("previously-rejected")?;
    let old = crate::commit_assessment::validate_commit_candidates(
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
    assert!(assess_commit_candidates(&fixture.context, std::slice::from_ref(&subject)).is_err());
    assert_eq!(subject.validation_state()?, ValidationState::Invalid);
    assert_eq!(subject.validation_findings()?, old.violations);
    assert!(crate::commit_assessment::validate_commit_candidates(
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
    for rule in [CoreValidationRuleName::SchemaDependenciesAcyclic] {
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
    let report = crate::commit_assessment::validate_commit_candidates(
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
fn scoped_installation_uses_the_same_distinct_candidate_check_as_commit() -> Result<(), HolonError>
{
    let fixture = Fixture::new()?;
    let candidate = fixture.staged_subject("repeated")?;
    let candidates = [candidate.clone(), candidate.clone()];
    let expected =
        crate::commit_assessment::require_distinct_live_candidates(&candidates).unwrap_err();
    let Err(actual) =
        PreparedAssessment::from_scope(&candidates, CommitValidationReport::default())
    else {
        panic!("repeated candidates must reject before installation")
    };
    assert_eq!(actual, expected);
    assert_eq!(candidate.validation_state()?, ValidationState::ValidationRequired);
    Ok(())
}

#[test]
fn constructed_saved_packages_validate_without_schema_backend_reads() -> Result<(), HolonError> {
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let mut subject = fixture.staged_subject("prepared-instance")?;
    subject.with_property_value("Title", "ready")?;
    let candidates = vec![subject];
    let roots = crate::commit_assessment::ReadinessContext::resolve(&fixture.context)
        .map_err(|e| HolonError::CommitFailure(e.to_string()))?;
    let constructed = crate::commit_assessment::construct_assessment(
        &fixture.context,
        &candidates,
        &roots,
        ValidationCollector::default(),
    )?;
    let relationship_reads_before = fixture.backend_relationship_reads();
    let node_reads_before = fixture.backend_node_reads();
    let report = constructed.assess(|_| {})?.0.into_report();
    assert!(report.is_accepted(), "{:?}", report);
    assert_eq!(
        relationship_reads_before,
        fixture.backend_relationship_reads(),
        "schema query after construction"
    );
    assert_eq!(node_reads_before, fixture.backend_node_reads(), "node fetch after construction");
    Ok(())
}

#[test]
fn profiling_observes_preparation_without_changing_the_validation_report() -> Result<(), HolonError>
{
    let fixture = readiness_fixture()?.saved_snapshot()?;
    let mut subject = fixture.staged_subject("profiled-instance")?;
    subject.with_property_value("Title", "ready")?;
    let candidates = vec![subject];
    let expected = assess_commit_candidates(&fixture.context, &candidates)?.report;
    let mut events = Vec::new();
    let actual =
        crate::validate_commit_candidates_with_profile(&fixture.context, &candidates, |event| {
            events.push(event)
        })?;
    assert_eq!(actual, expected);
    assert!(actual.is_accepted());
    let phases: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            crate::AssessmentEvent::Phase(phase) => Some(*phase),
            _ => None,
        })
        .collect();
    assert_eq!(
        phases,
        vec![
            crate::AssessmentPhase::Construction,
            crate::AssessmentPhase::ReadinessResolution,
            crate::AssessmentPhase::SubjectPreparation,
            crate::AssessmentPhase::SchemaWorksetPreparation,
            crate::AssessmentPhase::PackageDiscovery,
            crate::AssessmentPhase::PreparedProducts,
            crate::AssessmentPhase::PackageValidation,
            crate::AssessmentPhase::InstanceValidation,
        ]
    );
    let metrics: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            crate::AssessmentEvent::Preparation(metrics) => Some(metrics),
            _ => None,
        })
        .collect();
    assert_eq!(metrics.len(), 1);
    let metrics = metrics[0];
    assert_eq!(metrics.package_roots, 1);
    assert_eq!(metrics.subjects, 1);
    assert_eq!(metrics.affected_schemas, 0);
    assert!(metrics.distinct_content > 1);
    assert!(metrics.identity_role_visits >= metrics.distinct_content);
    assert_eq!(metrics.contract_consumers, 1);
    assert!(metrics.binding_consumers > 0);
    assert!(metrics.binding_consumers <= metrics.distinct_content);
    let described_by_requests: usize = events
        .iter()
        .filter_map(|event| match event {
            crate::AssessmentEvent::PreparationEdge { edge, requests, .. }
                if edge == "DescribedBy" =>
            {
                Some(*requests)
            }
            _ => None,
        })
        .sum();
    assert_eq!(described_by_requests, metrics.identity_role_visits);
    let typed_visits: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            crate::AssessmentEvent::PreparationVisitFinished { type_name, distinct_content } => {
                Some((type_name, *distinct_content))
            }
            _ => None,
        })
        .collect();
    assert_eq!(typed_visits.len(), metrics.identity_role_visits);
    assert_eq!(
        typed_visits.iter().filter(|(_, distinct)| *distinct).count(),
        metrics.distinct_content
    );
    assert!(typed_visits.iter().all(|(name, _)| !name.is_empty()));
    let mut active = None;
    let mut contracts = 0;
    let mut prefetches = 0;
    for event in &events {
        if let crate::AssessmentEvent::PreparationOperation { operation, role, starting } = event {
            if *starting {
                assert!(
                    active.replace((*operation, *role)).is_none(),
                    "operation spans must not overlap"
                );
            } else {
                assert_eq!(active.take(), Some((*operation, *role)));
                if *operation == "EffectiveContract" {
                    contracts += 1;
                }
                if *operation == "MembershipPrefetch" {
                    prefetches += 1;
                }
            }
        }
    }
    assert!(active.is_none());
    assert_eq!(contracts, metrics.distinct_content);
    assert!(prefetches > 0, "saved definitions must expose eager prefetch cost");
    Ok(())
}

#[test]
fn affected_saved_schema_and_mixed_candidates_need_no_reads_after_construction(
) -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    fixture.node("Owner")?;
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link("Owner", CoreRelationshipTypeName::DescribedBy, "Schema.HolonType")?;
    fixture.link("Contract", CoreRelationshipTypeName::ComponentOf, "Owner")?;
    fixture.link(
        "Contract",
        CoreRelationshipTypeName::DescribedBy,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let fixture = fixture.saved_snapshot()?;
    let descriptor = fixture.stage_update_of("Contract")?;
    let mut subject = fixture.staged_subject("mixed-instance")?;
    subject.with_property_value("Title", "ready")?;
    let candidates = vec![descriptor, subject];
    let roots = crate::commit_assessment::ReadinessContext::resolve(&fixture.context)
        .map_err(|e| HolonError::CommitFailure(e.to_string()))?;
    let constructed = crate::commit_assessment::construct_assessment(
        &fixture.context,
        &candidates,
        &roots,
        ValidationCollector::default(),
    )?;
    let relationship_reads_before = fixture.backend_relationship_reads();
    let node_reads_before = fixture.backend_node_reads();
    let report = constructed.assess(|_| {})?.0.into_report();
    assert!(!report.is_accepted(), "fixture's incomplete descriptor must retain its findings");
    assert_eq!(
        relationship_reads_before,
        fixture.backend_relationship_reads(),
        "schema query after construction"
    );
    assert_eq!(node_reads_before, fixture.backend_node_reads(), "node fetch after construction");
    Ok(())
}

#[test]
fn schema_dependency_cycles_do_not_read_saved_memberships() -> Result<(), HolonError> {
    let mut fixture = schema_fixture()?;
    fixture.link("A", CoreRelationshipTypeName::DependsOn, "B")?;
    fixture.link("B", CoreRelationshipTypeName::DependsOn, "A")?;
    fixture.link("A", CoreRelationshipTypeName::Components, "Member")?;
    fixture.link("A", CoreRelationshipTypeName::Rules, "OwnedRule")?;
    describe_schemas(&mut fixture)?;
    let fixture = fixture.saved_snapshot_with_read_errors(&[
        ("Member", HolonError::InvalidState("saved member must not be assessed".into())),
        ("OwnedRule", HolonError::InvalidState("saved rule must not be assessed".into())),
    ])?;
    let mut collector = ValidationCollector::default();
    let workset = SchemaWorkset::prepare(
        &fixture.context,
        &[fixture.nodes["A"].clone()],
        &[],
        &mut collector,
    )?;
    assert!(workset.schemas[0].staged_components.is_empty());
    assert!(workset.schemas[0].staged_rules.is_empty());
    let cycles = dependency_cycles(&fixture.context, &workset.schemas)?;
    assert!(cycles[&DefinitionIdentity::for_reference(&fixture.nodes["A"], &fixture.context)?]
        .is_some());
    // Package construction must not rediscover members excluded by the workset.
    crate::descriptor_package::DescriptorPackages::construct(&fixture.context, &[], &[], &workset)?;
    for edge in [CoreRelationshipTypeName::Components, CoreRelationshipTypeName::Rules] {
        assert_eq!(fixture.backend_reads_for(&edge), 0, "unexpected {edge:?} scan");
    }
    Ok(())
}

#[test]
fn stale_cross_schema_dependency_binding_fails_closed() -> Result<(), HolonError> {
    let mut fixture = readiness_fixture()?;
    let rule = CoreValidationRuleName::CrossSchemaDependenciesDeclared;
    assert!(crate::registries::registration(rule).implementation.is_none());
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::InstanceProperties,
        "Key.PropertyType",
    )?;
    fixture.node(rule.as_str())?;
    fixture.link(
        rule.as_str(),
        CoreRelationshipTypeName::DescribedBy,
        "HolonValidationRule.HolonType",
    )?;
    fixture.link(
        "Schema.HolonType",
        CoreRelationshipTypeName::ValidationBindings,
        rule.as_str(),
    )?;
    let fixture = fixture.saved_snapshot()?;
    let mut schema = fixture.context.mutation().stage_new_holon(
        fixture.context.mutation().new_holon(Some(MapString("StaleCoreSchema".into())))?,
    )?;
    schema.with_descriptor(fixture.nodes["Schema.HolonType"].clone())?;
    let assessment = assess_commit_candidates(&fixture.context, &[schema.clone()])?;
    assert_eq!(assessment.report.violation_count(), 1, "{:#?}", assessment.report.violations);
    let finding = &assessment.report.violations[0];
    assert_eq!(finding.kind, CommitValidationViolationKind::UnsupportedValidationRule);
    assert_eq!(finding.rule_key.as_deref(), Some(rule.as_str()));
    assert_eq!(finding.subject, path(&schema.into()));
    assert_eq!(
        finding.descriptor_identity.as_deref(),
        Some(fixture.nodes["Schema.HolonType"].reference_id_string().as_str())
    );
    assert!(assessment.observations.discovered_rule_keys.contains(rule.as_str()));
    assert!(!assessment.observations.dispatched_rule_keys.contains(rule.as_str()));
    // Pre-production stores must use the regenerated bootstrap: a stale mandatory
    // binding remains fail-closed even though its native handler has been retired.
    Ok(())
}
