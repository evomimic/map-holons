use super::*;
use crate::key_evidence::prepare_key_evidence;
use holons_core::{
    core_shared_objects::holon::{StagedState, ValidationState},
    LineageIdentity, StagedReference,
};

fn keyed_fixture(versions: &[(&str, &str, Option<&str>)]) -> Result<Fixture, HolonError> {
    let mut f = readiness_fixture()?;
    f.node("Predecessor.KeyTestRelationship")?
        .with_property_value("TypeName", "Predecessor")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    f.link(
        "Predecessor.KeyTestRelationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType",
    )?;
    for edge in [CoreRelationshipTypeName::SourceType, CoreRelationshipTypeName::TargetType] {
        f.link("Predecessor.KeyTestRelationship", edge, "Contract")?;
    }
    f.link(
        "Contract",
        CoreRelationshipTypeName::InstanceRelationships,
        "Predecessor.KeyTestRelationship",
    )?;
    for (name, key, _) in versions {
        let mut node = f.node(name)?;
        for property in ["TypeName", "IsAbstractType", "IsValueRequired"] {
            node.remove_property_value(property)?;
        }
        node.with_property_value("Key", *key)?.with_property_value("Title", "ready")?;
        f.link(name, CoreRelationshipTypeName::DescribedBy, "Contract")?;
    }
    let roots: Vec<_> =
        versions.iter().filter_map(|(name, _, root)| root.map(|root| (*name, root))).collect();
    f.saved_snapshot_with_lineages(&roots)
}
fn create(f: &Fixture, key: &str) -> Result<StagedReference, HolonError> {
    let mut subject = f.staged_subject(key)?;
    subject.with_property_value("Title", "ready")?;
    Ok(subject)
}
fn key_findings(report: &CommitValidationReport) -> Vec<&core_types::CommitValidationViolation> {
    report.violations.iter().filter(|v| matches!(&v.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-KEY-006")).collect()
}

#[test]
fn saved_and_live_collisions_preserve_binding_and_declaring_identity() -> Result<(), HolonError> {
    let f = keyed_fixture(&[("Source", "A", None)])?;
    let claim = create(&f, "A")?;
    let report = validate_commit_candidates(&f.context, &[claim.clone()])?;
    assert_eq!(report.violation_count(), 1, "{:#?}", report.violations);
    let finding = key_findings(&report)[0];
    assert_eq!(finding.rule_key.as_deref(), Some(CoreValidationRuleName::KeyUniqueness.as_str()));
    assert_eq!(
        finding.descriptor_identity.as_deref(),
        Some(f.nodes["HolonType.TypeDescriptor"].full_reference_id_string().as_str())
    );
    assert!(finding.message.contains("A"));
    assert_eq!(claim.validation_state()?, ValidationState::Invalid);

    // This fixture proves live collisions with empty saved evidence. The guest service test
    // covers absent-Space behavior during bootstrap and outside bootstrap.
    let f = readiness_fixture()?;
    f.context.enable_bootstrap_provisioning();
    let a = create(&f, "FirstBootstrapClaim")?;
    let b = create(&f, "FirstBootstrapClaim")?;
    let mut queries = 0;
    let report = validate_commit_candidates_with_profile(&f.context, &[a, b], |event| {
        if matches!(
            event,
            AssessmentEvent::PreparationOperation { role: "key-evidence", starting: true, .. }
        ) {
            queries += 1;
        }
    })?;
    assert_eq!(key_findings(&report).len(), 2);
    assert_eq!(queries, 1);
    Ok(())
}

#[test]
fn same_lineage_siblings_pass_but_same_key_continuations_check_other_lineages(
) -> Result<(), HolonError> {
    for collision in [false, true] {
        let versions = if collision {
            vec![("Source", "A", None), ("Other", "A", None)]
        } else {
            vec![
                ("Source", "A", None),
                ("V2", "A", Some("Source")),
                ("Sibling", "A", Some("Source")),
            ]
        };
        let f = keyed_fixture(&versions)?;
        let a = f.stage_update_of_with("Source", |_| Ok(()))?;
        let b = f.stage_update_of_with("Source", |_| Ok(()))?;
        let report = validate_commit_candidates(&f.context, &[a.clone(), b.clone()])?;
        assert_eq!(
            key_findings(&report).len(),
            if collision { 2 } else { 0 },
            "{:#?}",
            report.violations
        );
        assert_eq!(report.is_accepted(), !collision);
        assert_ne!(a.temporary_id(), b.temporary_id());
    }
    Ok(())
}

#[test]
fn historical_keys_and_cross_lineage_swaps_never_release_claims() -> Result<(), HolonError> {
    let f = keyed_fixture(&[("Source", "A", None), ("V2", "B", Some("Source"))])?;
    let a = create(&f, "A")?;
    let b = create(&f, "B")?;
    let report = validate_commit_candidates(&f.context, &[a.clone(), b.clone()])?;
    assert_eq!(key_findings(&report).len(), 2);
    a.abandon_staged_changes(&f.context)?;
    b.abandon_staged_changes(&f.context)?;
    // Returning to an earlier key in the same lineage remains valid.
    let mut returning = f.stage_update_of("V2")?;
    returning.with_property_value("Key", "A")?;
    assert!(validate_commit_candidates(&f.context, &[returning])?.is_accepted());

    let f = keyed_fixture(&[("First", "A", None), ("Second", "B", None)])?;
    let mut a = f.stage_update_of("First")?;
    let mut b = f.stage_update_of("Second")?;
    a.with_property_value("Key", "B")?;
    b.with_property_value("Key", "A")?;
    assert_eq!(a.staged_state()?, StagedState::ForUpdateNewVersion);
    let report = validate_commit_candidates(&f.context, &[a, b])?;
    assert_eq!(key_findings(&report).len(), 2);
    Ok(())
}

#[test]
fn new_clones_collide_but_intermediate_abandoned_and_different_complete_keys_do_not(
) -> Result<(), HolonError> {
    let f = keyed_fixture(&[("Source", "A", None)])?;
    let clone = f.context.mutation().stage_new_from_clone(f.nodes["Source"].clone(), "A".into())?;
    assert!(matches!(
        LineageIdentity::for_reference(&clone.clone().into(), &f.context)?,
        LineageIdentity::New(_)
    ));
    assert_eq!(key_findings(&validate_commit_candidates(&f.context, &[clone.clone()])?).len(), 1);
    clone.abandon_staged_changes(&f.context)?;

    let mut intermediate = create(&f, "Temporary")?;
    intermediate.with_property_value("Key", "UncommittedIntermediate")?;
    intermediate.with_property_value("Key", "Final")?;
    let abandoned = create(&f, "Unreserved")?;
    abandoned.abandon_staged_changes(&f.context)?;
    let report = validate_commit_candidates(
        &f.context,
        &[
            intermediate,
            create(&f, "Temporary")?,
            create(&f, "UncommittedIntermediate")?,
            create(&f, "Unreserved")?,
            create(&f, "A__1")?,
        ],
    )?;
    assert!(report.is_accepted(), "{:#?}", report.violations);
    Ok(())
}

#[test]
fn evidence_is_reused_once_per_exact_key_and_skipped_for_non_claims() -> Result<(), HolonError> {
    let f = keyed_fixture(&[("Source", "A", None)])?;
    let a = create(&f, "New")?;
    let b = create(&f, "New")?;
    let mut events = Vec::new();
    let claims =
        prepare_key_evidence(&f.context, &[a.clone(), b.clone()], &mut |e| events.push(e))?;
    assert_eq!(f.lineage_lookup_reads(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &claims[&a.temporary_id()].evidence,
        &claims[&b.temporary_id()].evidence
    ));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                AssessmentEvent::PreparationOperation { role: "key-evidence", starting: true, .. }
            ))
            .count(),
        1
    );
    let no_op = f.stage_update_of("Source")?;
    let graph = f.stage_update_of("Source")?;
    {
        let rc = graph.get_holon_to_commit(&f.context)?;
        let mut guard = rc.write().unwrap();
        let Holon::Staged(staged) = &mut *guard else { unreachable!() };
        staged.note_relationship_mutation(false)?;
    }
    let mut keyless = create(&f, "KeylessInput")?;
    keyless.remove_property_value("Key")?;
    let before = f.lineage_lookup_reads();
    assert!(prepare_key_evidence(
        &f.context,
        &[no_op.clone(), graph.clone(), keyless.clone()],
        &mut |_| {}
    )?
    .is_empty());
    let assessment = assess_commit_candidates(&f.context, &[no_op, graph, keyless])?;
    assert!(assessment.report.is_accepted(), "{:#?}", assessment.report.violations);
    assert_eq!(before, f.lineage_lookup_reads());
    Ok(())
}

#[test]
fn committed_nursery_claims_cover_a_missing_saved_key_index_entry() -> Result<(), HolonError> {
    let f = keyed_fixture(&[("PublishedRecord", "PublishedKey", None)])?;
    f.remove_saved_key_association("PublishedKey");
    let published = create(&f, "PublishedKey")?;
    // Simulate Pass 1's publication, before keyed Owns maintenance. The committed record
    // is readable, but no saved association exists for the staged candidate's final key.
    {
        let rc = published.get_holon_to_commit(&f.context)?;
        let mut guard = rc.write().unwrap();
        let Holon::Staged(staged) = &mut *guard else { unreachable!() };
        staged.prepare_full_relationship_commit_scope()?;
        staged.to_committed(f.nodes["PublishedRecord"].holon_id()?.local_id().clone())?;
    }
    let claim = create(&f, "PublishedKey")?;
    let report = validate_commit_candidates(&f.context, &[claim])?;
    assert_eq!(key_findings(&report).len(), 1);
    assert!(
        published.validation_findings()?.is_empty(),
        "persisted entries receive no new outcome"
    );
    Ok(())
}

#[test]
fn descriptor_key_failure_blocks_only_explicit_new_definition_consumers() -> Result<(), HolonError>
{
    let mut f = super::required_anchors::described_anchor_fixture()?;
    for (name, parent) in [
        ("LocalString", "StringValueType.ValueType"),
        ("DependentProperty", "PropertyType.TypeDescriptor"),
    ] {
        f.node(name)?.with_property_value("DefinesInstanceTypeKind", false)?;
        f.link(name, CoreRelationshipTypeName::Extends, parent)?;
        f.link(name, CoreRelationshipTypeName::ComponentOf, "AnchorOwner")?;
        if name == "DependentProperty" {
            f.link(name, CoreRelationshipTypeName::ValueType, "LocalString")?;
        }
        f.link(name, CoreRelationshipTypeName::DescribedBy, "MetaHolonType.MetaTypeDescriptor")?;
    }
    let mut local = f.nodes["LocalString"].clone();
    local.with_property_value("Key", "DefinitionCollision")?;
    let other = create(&f, "DefinitionCollision")?;
    let HolonReference::Staged(local) = local else { unreachable!() };
    let HolonReference::Staged(dependent) = f.nodes["DependentProperty"].clone() else {
        unreachable!()
    };
    for reversed in [false, true] {
        let mut candidates = vec![local.clone(), other.clone(), dependent.clone()];
        if reversed {
            candidates.reverse();
        }
        let report = validate_commit_candidates(&f.context, &candidates)?;
        assert_eq!(key_findings(&report).len(), 2, "{:#?}", report.violations);
        let local_findings = local.validation_findings()?;
        assert_eq!(local_findings.len(), 1, "{local_findings:#?}");
        assert!(
            matches!(&local_findings[0].kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-KEY-006")
        );
        assert!(dependent.validation_findings()?.iter().any(
            |finding| finding.kind == CommitValidationViolationKind::UnresolvedLocalDependency
        ));
    }
    Ok(())
}

#[test]
fn key_evidence_read_failure_preserves_all_prior_outcomes() -> Result<(), HolonError> {
    let f = keyed_fixture(&[("Source", "A", None)])?;
    let rejected = create(&f, "A")?;
    assert_eq!(
        key_findings(&validate_commit_candidates(&f.context, &[rejected.clone()])?).len(),
        1
    );
    let before = (rejected.validation_state()?, rejected.validation_findings()?);
    let independent = create(&f, "Independent")?;
    let independent_before = (independent.validation_state()?, independent.validation_findings()?);
    f.fail_lineage_lookup(HolonError::InvalidState("key-evidence failure".into()));
    assert!(
        matches!(validate_commit_candidates(&f.context, &[rejected.clone(), independent.clone()]), Err(HolonError::InvalidState(message)) if message == "key-evidence failure")
    );
    assert_eq!((rejected.validation_state()?, rejected.validation_findings()?), before);
    assert_eq!(
        (independent.validation_state()?, independent.validation_findings()?),
        independent_before
    );
    Ok(())
}

#[test]
fn repeated_saved_associations_count_as_one_conflicting_lineage() -> Result<(), HolonError> {
    let f = keyed_fixture(&[
        ("Source", "A", None),
        ("V2", "A", Some("Source")),
        ("Sibling", "A", Some("Source")),
    ])?;
    let roots = f.context.lookup().get_saved_lineage_roots_by_key(&"A".into())?;
    assert_eq!(roots.len(), 3, "fixture must retain repeated association rows");
    assert!(roots.iter().all(|root| root == &roots[0]));
    let candidate = create(&f, "A")?;
    let claims = prepare_key_evidence(&f.context, &[candidate.clone()], &mut |_| {})?;
    let claim = &claims[&candidate.temporary_id()];
    assert_eq!(claim.evidence.conflicting_lineages(&claim.lineage).len(), 1);
    let report = validate_commit_candidates(&f.context, &[candidate])?;
    assert_eq!(key_findings(&report).len(), 1);
    let message = &key_findings(&report)[0].message;
    assert!(message.starts_with("Key A for lineage staged "), "{message}");
    for debug_wrapper in ["MapString(", "Saved(", "LocalId(", "New("] {
        assert!(!message.contains(debug_wrapper), "{message}");
    }
    Ok(())
}
