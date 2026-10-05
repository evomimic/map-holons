//! Conductor-free checks of dance fixture construction and authoring-time validation.

mod fixture_cases;

use fixture_cases::*;
use holons_test::{DanceTestStep, ExpectedCommitStatus, ExpectedDisposition, TestHolonState};
use pretty_assertions::assert_eq;

/// Exercises fixture coverage gates without starting a conductor or dispatching dances.
#[test]
fn all_fixtures_author_without_a_conductor() {
    abandon_staged_changes_fixture::simple_abandon_staged_changes_fixture().unwrap();
    bootstrap_operational_schema_fixture::bootstrap_operational_schema_fixture().unwrap();
    commit_competition_fixture::commit_competition_retry_fixture().unwrap();
    commit_competition_fixture::commit_graph_only_competition_fixture().unwrap();
    commit_competition_fixture::commit_branch_across_transactions_fixture().unwrap();
    commit_disposition_fixture::commit_no_action_fixture().unwrap();
    commit_disposition_fixture::commit_mixed_dispositions_fixture().unwrap();
    commit_disposition_fixture::commit_same_key_dispositions_fixture().unwrap();
    commit_disposition_fixture::commit_stale_graph_only_declaration_fixture().unwrap();
    commit_disposition_fixture::commit_unsupported_pass_one_fixture().unwrap();
    commit_lineage_fixture::commit_sequential_lineage_fixture().unwrap();
    commit_lineage_fixture::commit_non_root_lineage_fixture().unwrap();
    commit_schema_fixture::commit_unstaged_schema_finding_fixture().unwrap();
    commit_schema_fixture::commit_schema_cycle_fixture().unwrap();
    commit_strict_contract_fixture::commit_strict_contract_fixture().unwrap();
    commit_validation_fixture::commit_validation_fixture().unwrap();
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
    load_holons_internal_fixture::loader_incremental_fixture().unwrap();
    load_inverse_oriented_book_person_instances_fixture::load_inverse_oriented_book_person_instances_fixture().unwrap();
    query_qry1_scaffold_fixture::query_qry1_scaffold_fixture().unwrap();
    query_qry2_seed_expand_fixture::query_qry2_seed_expand_fixture().unwrap();
    query_qry4a_order_paginate_fixture::query_qry4a_order_paginate_fixture().unwrap();
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
