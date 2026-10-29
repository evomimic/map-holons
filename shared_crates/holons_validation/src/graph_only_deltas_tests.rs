use super::*;
use crate::{assessment_support::path, graph_only_deltas::classify_graph_only_delta_overlaps};
use holons_core::{
    core_shared_objects::{
        holon::{RelationshipCommitScope, StagedState},
        WriteableHolonState,
    },
    SmartReference, StagedReference,
};
use type_names::ToRelationshipName;

fn graph_fixture(lineage: bool, existing: bool) -> Result<Fixture, HolonError> {
    graph_fixture_with_binding(if lineage { "Lineage" } else { "Version" }, existing)
}

fn graph_fixture_with_binding(binding: &str, existing: bool) -> Result<Fixture, HolonError> {
    let mut f = readiness_fixture()?;
    for key in ["Source", "X", "Y", "Xv2"] {
        f.node(key)?;
    }
    f.link("Source", CoreRelationshipTypeName::DescribedBy, "Contract")?;
    for name in ["TypeName", "IsAbstractType", "IsValueRequired"] {
        f.nodes.get_mut("Source").unwrap().remove_property_value(name)?;
    }
    f.nodes.get_mut("Source").unwrap().with_property_value("Title", "ready")?;
    f.node("R.Relationship")?
        .with_property_value("TypeName", "R")?
        .with_property_value("TargetBinding", BaseValue::EnumValue(MapEnumValue(binding.into())))?;
    f.link("R.Relationship", CoreRelationshipTypeName::Extends, "DeclaredRelationshipType")?;
    f.nodes
        .get_mut("R.Relationship")
        .unwrap()
        .with_property_value("IsDefinitional", false)?
        .with_property_value("AllowsDuplicates", true)?;
    f.link("R.Relationship", CoreRelationshipTypeName::SourceType, "Contract")?;
    f.link("R.Relationship", CoreRelationshipTypeName::TargetType, "HolonType.TypeDescriptor")?;
    f.nodes
        .get_mut("R.Relationship")
        .unwrap()
        .with_property_value("DefinesInstanceTypeKind", false)?;
    f.link("Contract", CoreRelationshipTypeName::InstanceRelationships, "R.Relationship")?;
    f.node("Predecessor.Relationship")?
        .with_property_value("TypeName", "Predecessor")?
        .with_property_value("DefinesInstanceTypeKind", false)?;
    f.link(
        "Predecessor.Relationship",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType",
    )?;
    f.link("Predecessor.Relationship", CoreRelationshipTypeName::SourceType, "Contract")?;
    f.link("Predecessor.Relationship", CoreRelationshipTypeName::TargetType, "Contract")?;
    f.link(
        "Contract",
        CoreRelationshipTypeName::InstanceRelationships,
        "Predecessor.Relationship",
    )?;
    if existing {
        let target = f.nodes["X"].clone();
        f.nodes.get_mut("Source").unwrap().add_related_holons("R", vec![target])?;
    }
    f.saved_snapshot_with_lineages(&[("Xv2", "X")])
}

/// Exercise the real staged mutators while bypassing descriptor policy: the classifier
/// must also defend loader/ungoverned assembly, with repeated occurrences intact.
fn mutate(
    f: &Fixture,
    candidate: &StagedReference,
    target: HolonReference,
    remove: bool,
) -> Result<(), HolonError> {
    let key = target.key()?;
    let rc = candidate.get_holon_to_commit(&f.context)?;
    let mut guard = rc.write().unwrap();
    let Holon::Staged(staged) = &mut *guard else { panic!("staged") };
    if remove {
        staged.remove_related_holons_with_keys(&"R".to_relationship_name(), vec![(target, key)])?;
    } else {
        staged.add_related_holons_with_keys("R".to_relationship_name(), vec![(target, key)])?;
    }
    staged.note_relationship_mutation(false)?;
    Ok(())
}
fn overlap(
    f: &Fixture,
    candidates: &[StagedReference],
) -> Result<Vec<core_types::CommitValidationViolation>, HolonError> {
    let mut collector = ValidationCollector::default();
    let bucket_reads = f.backend_reads_for_name(&"R".to_relationship_name());
    classify_graph_only_delta_overlaps(&f.context, candidates, None, &mut collector)?;
    assert_eq!(
        f.backend_reads_for_name(&"R".to_relationship_name()),
        bucket_reads,
        "overlap analysis must not reread the saved collection, even with cold descriptor caches"
    );
    Ok(collector.violations().to_vec())
}

#[test]
fn additions_compose_and_definition_or_cached_key_collisions_reject() -> Result<(), HolonError> {
    for (lineage, second, cached_key, rejected) in [
        (false, "Y", None, false),
        (false, "X", None, false),
        (true, "Xv2", None, true),
        (true, "Xv2", Some("X"), true),
        (false, "X", Some("cached-different"), true),
        (true, "X", None, false),
        (false, "Xv2", None, false),
        (false, "Xv2", Some("X"), false),
    ] {
        let f = graph_fixture(lineage, false)?;
        let a = f.stage_update_of("Source")?;
        let b = f.stage_update_of("Source")?;
        mutate(&f, &a, f.nodes["X"].clone(), false)?;
        let target = if let Some(key) = cached_key {
            SmartReference::new_with_properties(
                f.context.space_read_handle(),
                f.nodes[second].holon_id()?,
                core_types::PropertyMap::from([(
                    core_types::PropertyName("Key".into()),
                    BaseValue::StringValue(key.into()),
                )]),
            )
            .into()
        } else {
            f.nodes[second].clone()
        };
        mutate(&f, &b, target, false)?;
        // Warm descriptor inputs once; subsequent overlap checks must read no backend collection.
        let first = overlap(&f, &[a.clone(), b.clone()])?;
        let reads = f.backend_relationship_reads();
        let reverse = overlap(&f, &[b.clone(), a.clone()])?;
        assert_eq!(f.backend_relationship_reads(), reads);
        assert_eq!(first, reverse);
        assert_eq!(first.len(), if rejected { 2 } else { 0 }, "{first:#?}");
        if rejected {
            assert!(first.iter().all(|finding| finding
                .message
                .contains(&f.nodes["Source"].full_reference_id_string())));
            for subject in [&a, &b] {
                assert_eq!(first.iter().filter(|v| v.subject == path(&subject.into())).count(), 1);
            }
        }
    }
    Ok(())
}

#[test]
fn removals_repeats_and_outstanding_replay_reject_only_live_participants() -> Result<(), HolonError>
{
    for removals in [1, 2] {
        let f = graph_fixture(false, true)?;
        let a = f.stage_update_of("Source")?;
        let b = f.stage_update_of("Source")?;
        mutate(&f, &a, f.nodes["X"].clone(), true)?;
        mutate(&f, &b, f.nodes["X"].clone(), removals == 2)?;
        let findings = overlap(&f, &[a.clone(), b.clone()])?;
        assert_eq!(findings.len(), 2);
        // Committed graph-only deltas remain participants, with no new outcome of their own.
        {
            let rc = a.get_holon_to_commit(&f.context)?;
            let mut guard = rc.write().unwrap();
            let Holon::Staged(staged) = &mut *guard else { panic!("staged") };
            staged.prepare_touched_relationship_commit_scope()?;
            staged.to_committed(f.nodes["Source"].holon_id()?.local_id().clone())?;
            assert_eq!(staged.relationship_commit_scope(), RelationshipCommitScope::TouchedOnly);
        }
        let findings = overlap(&f, std::slice::from_ref(&b))?;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, path(&b.into()));
        assert!(findings[0].message.contains("outstanding replay from an incomplete Commit"));
        assert!(findings[0].message.contains(&HolonReference::from(&a).full_reference_id_string()));
    }
    // Existing occurrences are distinct from new duplicate-allowing additions.
    let f = graph_fixture(false, true)?;
    let a = f.stage_update_of("Source")?;
    let b = f.stage_update_of("Source")?;
    mutate(&f, &a, f.nodes["X"].clone(), false)?;
    mutate(&f, &b, f.nodes["X"].clone(), false)?;
    assert_eq!(overlap(&f, &[a, b])?.len(), 2);
    for initially_present in [false, true] {
        let f = graph_fixture(false, initially_present)?;
        let a = f.stage_update_of("Source")?;
        let b = f.stage_update_of("Source")?;
        mutate(&f, &a, f.nodes["X"].clone(), false)?;
        if !initially_present {
            mutate(&f, &a, f.nodes["X"].clone(), false)?;
        }
        mutate(&f, &a, f.nodes["X"].clone(), true)?;
        assert!(overlap(&f, std::slice::from_ref(&a))?.is_empty());
        mutate(&f, &b, f.nodes["Y"].clone(), false)?;
        assert_eq!(overlap(&f, &[a, b])?.len(), 2);
    }
    Ok(())
}

#[test]
fn committed_additions_compose_and_version_siblings_stay_independent() -> Result<(), HolonError> {
    let f = graph_fixture(false, false)?;
    let a = f.stage_update_of("Source")?;
    let b = f.stage_update_of("Source")?;
    let version = f.stage_update_of_with("Source", |_| Ok(()))?;
    mutate(&f, &a, f.nodes["X"].clone(), false)?;
    mutate(&f, &b, f.nodes["Y"].clone(), false)?;
    {
        let rc = a.get_holon_to_commit(&f.context)?;
        let mut guard = rc.write().unwrap();
        let Holon::Staged(staged) = &mut *guard else { panic!("staged") };
        staged.prepare_touched_relationship_commit_scope()?;
        staged.to_committed(f.nodes["Source"].holon_id()?.local_id().clone())?;
    }
    assert!(overlap(&f, &[b.clone(), version.clone()])?.is_empty());
    assert!(overlap(&f, &[version.clone(), b])?.is_empty());
    assert_eq!(version.staged_state()?, StagedState::ForUpdateNewVersion);
    assert!(HolonReference::from(&version)
        .related_holons("R")?
        .read()
        .unwrap()
        .get_members()
        .is_empty());
    Ok(())
}

#[test]
fn overlap_findings_remain_candidate_scoped_through_common_assessment() -> Result<(), HolonError> {
    let f = graph_fixture(false, true)?;
    let a = f.stage_update_of("Source")?;
    let b = f.stage_update_of("Source")?;
    let unaffected = f.stage_update_of("Source")?;
    mutate(&f, &a, f.nodes["X"].clone(), true)?;
    mutate(&f, &b, f.nodes["Y"].clone(), false)?;
    let candidates = [a.clone(), b.clone(), unaffected.clone()];
    let report = validate_commit_candidates(&f.context, &candidates)?;
    assert!(!report.is_accepted());
    for candidate in [&a, &b] {
        assert!(candidate.validation_findings()?.iter().any(|finding| matches!(&finding.kind,
            CommitValidationViolationKind::RuleViolation { code } if code == "UnsupportedGraphOnlyCombination")));
        assert_eq!(
            candidate.validation_state()?,
            holons_core::core_shared_objects::holon::ValidationState::Invalid
        );
    }
    assert_eq!(
        unaffected.validation_state()?,
        holons_core::core_shared_objects::holon::ValidationState::Validated,
        "{:#?}",
        unaffected.validation_findings()?
    );
    Ok(())
}

#[test]
fn undeclared_addition_overlaps_remain_semantic_findings() -> Result<(), HolonError> {
    let f = graph_fixture(false, false)?;
    let a = f.stage_update_of("Source")?;
    let b = f.stage_update_of("Source")?;
    let unaffected = f.stage_update_of("Source")?;
    for candidate in [&a, &b] {
        let rc = candidate.get_holon_to_commit(&f.context)?;
        let mut guard = rc.write().unwrap();
        let Holon::Staged(staged) = &mut *guard else { panic!("staged") };
        // Simulate ungoverned or reconstructed edits that bypass declaration checking.
        staged.add_related_holons_with_keys(
            "Undeclared".to_relationship_name(),
            vec![(f.nodes["X"].clone(), None)],
        )?;
        staged.note_relationship_mutation(false)?;
    }
    let report =
        validate_commit_candidates(&f.context, &[a.clone(), b.clone(), unaffected.clone()])?;
    assert!(!report.is_accepted());
    assert_eq!(report.violation_count(), 4, "{:#?}", report.violations);
    for candidate in [&a, &b] {
        let findings = candidate.validation_findings()?;
        assert_eq!(findings.len(), 2);
        for expected in ["UnsupportedGraphOnlyCombination", "UndeclaredRelationship"] {
            assert!(findings.iter().any(|finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code } if code == expected)));
        }
    }
    assert_eq!(
        unaffected.validation_state()?,
        holons_core::core_shared_objects::holon::ValidationState::Validated
    );
    Ok(())
}

#[test]
fn invalid_binding_is_still_an_error_and_preserves_prior_outcomes() -> Result<(), HolonError> {
    let f = graph_fixture_with_binding("Invalid", false)?;
    let a = f.stage_update_of("Source")?;
    let b = f.stage_update_of("Source")?;
    mutate(&f, &a, f.nodes["X"].clone(), false)?;
    mutate(&f, &b, f.nodes["Y"].clone(), false)?;
    let before_a = (a.validation_state()?, a.validation_findings()?);
    let before_b = (b.validation_state()?, b.validation_findings()?);
    assert!(matches!(validate_commit_candidates(&f.context, &[a.clone(), b.clone()]),
        Err(HolonError::InvalidParameter(message)) if message.contains("TargetBinding")));
    assert_eq!((a.validation_state()?, a.validation_findings()?), before_a);
    assert_eq!((b.validation_state()?, b.validation_findings()?), before_b);
    Ok(())
}
