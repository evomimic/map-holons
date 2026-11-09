//! Conductor-free checks of dance fixture construction and authoring-time validation.

mod fixture_cases;

use fixture_cases::*;
use holons_test::{DanceTestStep, ExpectedCommitStatus, ExpectedDisposition, TestHolonState};
use pretty_assertions::assert_eq;

/// Exercises fixture coverage gates without starting a conductor or dispatching dances.
#[test]
fn all_fixtures_author_without_a_conductor() {
    abandon_staged_changes_fixture::simple_abandon_staged_changes_fixture().unwrap();
    commit_sibling_fixture::commit_version_siblings_fixture().unwrap();
    commit_sibling_fixture::commit_no_op_and_graph_only_siblings_fixture().unwrap();
    commit_graph_only_fixture::commit_disjoint_graph_additions_fixture().unwrap();
    commit_incomplete_fixture::graph_overlap_retry_fixture().unwrap();
    commit_incomplete_fixture::key_index_retry_fixture().unwrap();
    commit_key_uniqueness_fixture::commit_historical_key_uniqueness_fixture().unwrap();
    commit_graph_only_fixture::commit_identical_graph_additions_fixture().unwrap();
    commit_graph_only_fixture::commit_graph_overlap_rejection_fixture().unwrap();
    commit_graph_only_fixture::commit_graph_then_version_fixture().unwrap();
    commit_graph_only_fixture::commit_version_then_graph_fixture().unwrap();
    commit_sibling_fixture::commit_branch_across_transactions_fixture().unwrap();
    commit_disposition_fixture::commit_no_action_fixture().unwrap();
    commit_disposition_fixture::commit_mixed_dispositions_fixture().unwrap();
    commit_disposition_fixture::commit_same_key_dispositions_fixture().unwrap();
    commit_lineage_fixture::commit_sequential_lineage_fixture().unwrap();
    commit_lineage_fixture::commit_non_root_lineage_fixture().unwrap();
    commit_schema_fixture::commit_cross_schema_dependency_gap_fixture().unwrap();
    commit_schema_fixture::commit_schema_cycle_fixture().unwrap();
    commit_schema_fixture::commit_schema_successor_membership_fixture().unwrap();
    commit_sibling_fixture::commit_book_title_exact_references_fixture().unwrap();
    commit_schema_fixture::commit_core_successor_fixture().unwrap();
    commit_schema_fixture::loader_core_successor_fixture().unwrap();
    commit_strict_contract_fixture::commit_strict_contract_fixture().unwrap();
    commit_validation_fixture::commit_validation_fixture().unwrap();
    attachment_defaults_fixture::attachment_defaults_fixture().unwrap();
    delete_holon_fixture::delete_holon_fixture().unwrap();
    ergonomic_add_remove_properties_fixture::ergonomic_add_remove_properties_fixture().unwrap();
    ergonomic_add_remove_related_holons_fixture::ergonomic_add_remove_related_holons_fixture()
        .unwrap();
    load_book_person_inverse_schema_fixture::load_book_person_inverse_schema_fixture().unwrap();
    load_book_person_inverse_schema_fixture::frozen_member_head_redirect_fixture().unwrap();
    load_book_person_inverse_schema_fixture::frozen_member_head_redirect_cross_tx_fixture()
        .unwrap();
    load_book_person_inverse_schema_fixture::cross_transaction_staged_target_diagnostic_fixture()
        .unwrap();
    load_inverse_oriented_book_person_instances_fixture::load_inverse_oriented_book_person_instances_fixture().unwrap();
    query_boundary_fixture::query_boundary_fixture().unwrap();
    query_qry1_scaffold_fixture::query_qry1_scaffold_fixture().unwrap();
    query_qry2_seed_expand_fixture::query_qry2_seed_expand_fixture().unwrap();
    query_qry4a_order_paginate_fixture::query_qry4a_order_paginate_fixture().unwrap();
    query_f1_filter_fixture::query_f1_filter_fixture().unwrap();
    query_f2_equality_fixture::query_f2_equality_fixture().unwrap();
    query_f3_boolean_fixture::query_f3_boolean_fixture().unwrap();
    simple_add_remove_properties_fixture::simple_add_remove_properties_fixture().unwrap();
    simple_add_remove_related_holons_fixture::simple_add_remove_related_holons_fixture().unwrap();
    simple_create_holon_fixture::simple_create_holon_fixture().unwrap();
    smartlink_commit_cache_fixture::smartlink_commit_cache_fixture().unwrap();
    stage_new_from_clone_fixture::stage_new_from_clone_fixture().unwrap();
    stage_new_version_fixture::stage_new_version_fixture().unwrap();
    transaction_lifecycle_fixture::transaction_lifecycle_fixture().unwrap();
}

#[test]
fn incomplete_fixture_authors_all_attempts_without_a_conductor() {
    let scenario = commit_incomplete_fixture::commit_incomplete_fixture().unwrap();
    assert_eq!(scenario.attempts.len(), 2);
    assert!(scenario.attempts[0].unchanged.is_none());
    assert!(scenario.attempts[1].unchanged.is_some());
    for sequence in &scenario.attempts {
        let unchanged_count = usize::from(sequence.unchanged.is_some());
        let attempts = &scenario.case.steps[sequence.first_attempt..sequence.first_attempt + 3];
        for (index, step) in attempts.iter().enumerate() {
            let DanceTestStep::Commit { candidates, retry_participants, expected_status, .. } =
                step
            else {
                panic!("three consecutive Commit attempts");
            };
            assert_eq!(candidates.len(), (if index == 0 { 2 } else { 0 }) + unchanged_count);
            assert_eq!(retry_participants.len(), if index == 0 { 0 } else { 1 });
            assert_eq!(
                *expected_status,
                if index == 2 {
                    ExpectedCommitStatus::Complete
                } else {
                    ExpectedCommitStatus::Incomplete
                }
            );
            if let Some(unchanged) = &sequence.unchanged {
                let candidate = candidates
                    .iter()
                    .find(|candidate| candidate.disposition == ExpectedDisposition::NoAction)
                    .unwrap();
                assert_eq!(candidate.staged_token, unchanged.staged);
                assert!(candidate.expected_new_errors.is_empty());
                assert_eq!(candidate.result_token.is_some(), index == 2);
                if index == 2 {
                    assert_eq!(
                        candidate.result_token.as_ref().unwrap().expected_id(),
                        unchanged.saved.expected_id()
                    );
                    assert_eq!(unchanged.saved.expected_snapshot().state(), TestHolonState::Saved);
                }
            } else {
                assert!(candidates.iter().all(|candidate| candidate.result_token.is_some()));
            }
        }
    }
}

#[test]
fn descriptor_step_replaces_targets_and_preserves_authored_values() {
    use holons_prelude::prelude::*;
    use holons_test::TestCaseInit;
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } =
        TestCaseInit::new("Descriptor expectation", "Replacement and explicit defaults");
    let mut targets = Vec::new();
    for key in ["old-descriptor", "new-descriptor"] {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone())).unwrap();
        targets.push(
            test_case
                .add_lookup_saved_holon_by_key_step(&mut fixture_holons, stub, key, None, None)
                .unwrap(),
        );
    }
    let key = MapString("subject".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone())).unwrap();
    let properties: PropertyMap = [("Authored".to_property_name(), true.to_base_value())].into();
    let subject = test_case
        .add_new_holon_step(&mut fixture_holons, source, properties, Some(key), None, None)
        .unwrap();
    let subject = test_case.add_stage_holon_step(&mut fixture_holons, subject, None, None).unwrap();
    let subject = test_case
        .add_add_related_holons_step(
            &mut fixture_holons,
            subject,
            CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
            vec![targets[0].clone()],
            None,
            None,
        )
        .unwrap();
    let defaults: PropertyMap = [
        ("Authored".to_property_name(), false.to_base_value()),
        ("Added".to_property_name(), false.to_base_value()),
    ]
    .into();
    let updated = test_case
        .add_with_descriptor_step(
            &mut fixture_holons,
            subject.clone(),
            targets[1].clone(),
            defaults.clone(),
            None,
            None,
        )
        .unwrap();
    let expected = updated.expected_snapshot();
    assert_eq!(expected.snapshot().property_value("Authored").unwrap(), Some(true.to_base_value()));
    assert_eq!(expected.snapshot().property_value("Added").unwrap(), Some(false.to_base_value()));
    let members = expected
        .snapshot()
        .related_holons(CoreRelationshipTypeName::DescribedBy)
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].key().unwrap(), Some(MapString("new-descriptor".into())));
    let original = subject.expected_snapshot();
    assert_eq!(original.snapshot().property_value("Added").unwrap(), None);
    assert_eq!(
        original
            .snapshot()
            .related_holons(CoreRelationshipTypeName::DescribedBy)
            .unwrap()
            .read()
            .unwrap()
            .get_members()[0]
            .key()
            .unwrap(),
        Some(MapString("old-descriptor".into()))
    );
    let repeated = test_case
        .add_with_descriptor_step(
            &mut fixture_holons,
            updated,
            targets[1].clone(),
            defaults,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        repeated.expected_snapshot().snapshot().into_model().unwrap().property_map,
        expected.snapshot().into_model().unwrap().property_map
    );
    test_case.finalize(&fixture_context, &fixture_holons).unwrap();
}

/// The correction scenario opts into bounded checks without changing successful Commit policy.
#[test]
fn validation_correction_declares_both_roots_and_descriptor_inverse_guards() {
    use holons_prelude::prelude::*;
    use holons_test::RejectionVerification;
    let case = commit_validation_fixture::commit_validation_fixture().unwrap();
    let commits: Vec<_> = case
        .steps
        .iter()
        .filter_map(|step| {
            if let DanceTestStep::Commit { rejection_verification, expected_status, .. } = step {
                Some((rejection_verification, expected_status))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(commits.len(), 2);
    let (RejectionVerification::FixtureScoped { subjects, unchanged_relationships }, status) =
        commits[0]
    else {
        panic!("first Commit must declare bounded rejection checks");
    };
    assert_eq!(*status, ExpectedCommitStatus::Rejected);
    let mut keys: Vec<_> = subjects
        .iter()
        .map(|token| token.expected_snapshot().snapshot().key().unwrap().unwrap().0)
        .collect();
    keys.sort();
    assert_eq!(keys, ["Book.ValidationCorrection", "Person.ValidationCorrection"]);
    let mut guards: Vec<_> = unchanged_relationships
        .iter()
        .map(|guard| {
            (
                guard.source.expected_snapshot().snapshot().key().unwrap().unwrap().0,
                guard.relationship.to_string(),
            )
        })
        .collect();
    guards.sort();
    assert_eq!(
        guards,
        vec![
            ("Book.HolonType".into(), "Instances".into()),
            ("Person.HolonType".into(), "Instances".into()),
        ]
    );
    assert!(matches!(
        commits[1],
        (RejectionVerification::ExistingPersistenceSnapshot, ExpectedCommitStatus::Complete)
    ));
}

/// Commit freezes provenance and memberships before later heads advance.
#[test]
fn commit_verification_uses_declared_fixture_worksets() {
    let case = commit_graph_only_fixture::commit_identical_graph_additions_fixture().unwrap();
    let commits: Vec<_> = case
        .steps
        .iter()
        .filter_map(|step| match step {
            DanceTestStep::Commit { candidates, .. } => Some(candidates),
            _ => None,
        })
        .collect();
    assert!(commits[0].iter().all(|candidate| candidate.saved_source.is_none()));
    let siblings = commits.last().unwrap();
    assert_eq!(siblings.len(), 2);
    assert_eq!(siblings[0].saved_source, siblings[1].saved_source);
    for candidate in *siblings {
        let relationship = candidate
            .relationships
            .iter()
            .find(|relationship| relationship.name.to_string() == "ReferencesProperty")
            .unwrap();
        assert!(relationship.before.is_empty());
        assert_eq!(relationship.after.len(), 1);
        assert!(candidate.relationships.iter().all(|relationship| !matches!(
            relationship.name.to_string().as_str(),
            "Predecessor" | "Successor"
        )));
    }
}

/// Commit scenarios use shared persisted checks rather than rediscovering their graph.
#[test]
fn audited_commit_cases_need_no_standalone_graph_step() {
    let cases = [
        commit_lineage_fixture::commit_sequential_lineage_fixture().unwrap(),
        commit_lineage_fixture::commit_non_root_lineage_fixture().unwrap(),
        commit_graph_only_fixture::commit_disjoint_graph_additions_fixture().unwrap(),
        commit_graph_only_fixture::commit_identical_graph_additions_fixture().unwrap(),
        commit_graph_only_fixture::commit_graph_overlap_rejection_fixture().unwrap(),
        commit_graph_only_fixture::commit_graph_then_version_fixture().unwrap(),
        commit_graph_only_fixture::commit_version_then_graph_fixture().unwrap(),
        commit_sibling_fixture::commit_version_siblings_fixture().unwrap(),
        commit_sibling_fixture::commit_no_op_and_graph_only_siblings_fixture().unwrap(),
        commit_sibling_fixture::commit_book_title_exact_references_fixture().unwrap(),
        commit_sibling_fixture::commit_branch_across_transactions_fixture().unwrap(),
        commit_schema_fixture::commit_schema_successor_membership_fixture().unwrap(),
        commit_incomplete_fixture::commit_incomplete_fixture().unwrap().case,
        commit_incomplete_fixture::graph_overlap_retry_fixture().unwrap().case,
    ];
    for case in cases {
        assert!(case.verify_committed_workset, "{} requires shared Commit graph checks", case.name);
        assert!(
            case.steps
                .iter()
                .all(|step| !matches!(step, DanceTestStep::VerifyPersistedGraph { .. })),
            "{} must verify its declared workset through Commit",
            case.name
        );
    }
}

/// A partial lookup stub cannot describe the member being removed; the operation still must.
#[test]
fn partial_lookup_removals_remain_explicit_commit_intent() {
    use holons_prelude::prelude::*;
    use holons_test::{ExpectedCommitCandidate, ExpectedMembershipEdit, TestCaseInit};
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Partial membership intent",
        "Keep explicit removal of an unknown stub member",
    );
    let mut tokens = Vec::new();
    for key in ["Book.PartialIntent", "Title.PropertyType", "Name.PropertyType"] {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone())).unwrap();
        tokens.push(
            test_case
                .add_lookup_saved_holon_by_key_step(&mut fixture_holons, stub, key, None, None)
                .unwrap(),
        );
    }
    let update = test_case
        .add_stage_new_version_step(
            &mut fixture_holons,
            tokens[0].clone(),
            None,
            MapInteger(1),
            None,
            None,
        )
        .unwrap();
    let update = test_case
        .add_remove_related_holons_step(
            &mut fixture_holons,
            update,
            "ReferencesProperty".to_relationship_name(),
            vec![tokens[1].clone()],
            None,
            None,
        )
        .unwrap();
    let update = test_case
        .add_add_related_holons_step(
            &mut fixture_holons,
            update,
            "ReferencesProperty".to_relationship_name(),
            vec![tokens[2].clone()],
            None,
            None,
        )
        .unwrap();
    test_case
        .add_commit_step_with_dispositions(
            &mut fixture_holons,
            ExpectedCommitStatus::Complete,
            vec![ExpectedCommitCandidate::new(update, ExpectedDisposition::GraphOnly)],
            vec![],
            None,
            None,
        )
        .unwrap();
    let DanceTestStep::Commit { candidates, .. } = test_case.steps.last().unwrap() else {
        unreachable!()
    };
    let relationship = candidates
        .iter()
        .flat_map(|candidate| &candidate.relationships)
        .find(|relationship| relationship.name.to_string() == "ReferencesProperty")
        .unwrap();
    assert!(relationship.before.is_empty(), "the saved source is a partial lookup");
    assert!(relationship
        .edits
        .iter()
        .any(|edit| matches!(edit, ExpectedMembershipEdit::Remove(targets) if targets.len() == 1)));
    assert!(relationship
        .edits
        .iter()
        .any(|edit| matches!(edit, ExpectedMembershipEdit::Add(targets) if targets.len() == 1)));
}

#[test]
fn abandoned_targets_are_removed_from_pending_commit_intent() {
    use fixture_cases::described_instance_helpers::add_described_instance;
    use holons_prelude::prelude::*;
    use holons_test::harness::helpers::{BOOK_DESCRIPTOR_KEY, PERSON_DESCRIPTOR_KEY};
    use holons_test::{ExpectedMembershipEdit, TestCaseInit};
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } =
        TestCaseInit::new("Abandon related intent", "Prune pending relationship intent");
    let book = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.PrunedIntent",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )
    .unwrap();
    let person = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Person.PrunedIntent",
        "Name",
        PERSON_DESCRIPTOR_KEY,
    )
    .unwrap();
    test_case
        .add_add_related_holons_step(
            &mut fixture_holons,
            book,
            "AuthoredBy".to_relationship_name(),
            vec![person.clone()],
            None,
            None,
        )
        .unwrap();
    test_case.add_abandon_staged_changes_step(&mut fixture_holons, person, None, None).unwrap();
    test_case
        .add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)
        .unwrap();
    let DanceTestStep::Commit { candidates, .. } = test_case.steps.last().unwrap() else {
        unreachable!()
    };
    let relationship =
        candidates[0].relationships.iter().find(|r| r.name.to_string() == "AuthoredBy").unwrap();
    assert!(relationship.after.is_empty());
    assert!(relationship
        .edits
        .iter()
        .all(|edit| matches!(edit, ExpectedMembershipEdit::Add(targets) if targets.is_empty())));
}
