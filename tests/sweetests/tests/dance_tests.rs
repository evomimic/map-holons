//! MAP Dance Test Cases
//!
//! The functions in this file are used in conjunction with Rust rstest test fixtures.
//! Tracing is initialized automatically by the test harness.
//! export RUST_LOG to the desired client-side tracing level to include in output.
//! export WASM_LOG to the desired guest-side tracing level to include in output.
//! In increasing level of detail:
//! error, warn, info, debug, trace

//! Examples:

//! To show DEBUG level trace messages on the client-side and WARN level trace messages on the guest-side:
//! export RUST_LOG=debug
//! export WASM_LOG=warn

//! To show INFO level trace messages on the client-side and DEBUG level trace messages on the guest-side:
//! export RUST_LOG=info
//! export WASM_LOG=debug

mod execution_steps;
mod fixture_cases;

use execution_steps::dance_test_runner::{run_dance_test_suite, DanceTestSuite};
use rstest::*;

use fixture_cases::abandon_staged_changes_fixture::*;
use fixture_cases::bootstrap_operational_schema_fixture::*;
use fixture_cases::commit_competition_fixture::*;
use fixture_cases::commit_disposition_fixture::*;
use fixture_cases::commit_lineage_fixture::*;
use fixture_cases::commit_schema_fixture::*;
use fixture_cases::commit_strict_contract_fixture::*;
use fixture_cases::commit_validation_fixture::*;
use fixture_cases::delete_holon_fixture::*;
use fixture_cases::ergonomic_add_remove_properties_fixture::*;
use fixture_cases::ergonomic_add_remove_related_holons_fixture::*;
use fixture_cases::load_book_person_inverse_schema_fixture::*;
use fixture_cases::load_holons_internal_fixture::*;
use fixture_cases::load_inverse_oriented_book_person_instances_fixture::*;
use fixture_cases::query_qry1_scaffold_fixture::*;
use fixture_cases::query_qry2_seed_expand_fixture::*;
use fixture_cases::query_qry4a_order_paginate_fixture::*;
use fixture_cases::simple_add_remove_properties_fixture::*;
use fixture_cases::simple_add_remove_related_holons_fixture::*;
use fixture_cases::simple_create_holon_fixture::*;
use fixture_cases::smartlink_commit_cache_fixture::*;
use fixture_cases::stage_new_from_clone_fixture::*;
use fixture_cases::stage_new_version_fixture::*;
use fixture_cases::transaction_lifecycle_fixture::*;

/// Dance Sweettests share a bootstrapped runtime within each suite. Every
/// scenario still receives its own transaction and fixture execution registry.
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p dances --test dance_tests
///      set RUST_LOG to enable client-side (i.e., test code) tracing
///      set WASM_LOG to enable guest-side (i.e., zome code) tracing
///
#[rstest]
#[case::pristine_bootstrap_and_loader(pristine_bootstrap_and_loader_suite())]
#[case::runtime_behavior_matrix(runtime_behavior_matrix_suite())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_test_suites(#[case] suite: DanceTestSuite) {
    run_dance_test_suite(suite).await;
}

fn pristine_bootstrap_and_loader_suite() -> DanceTestSuite {
    DanceTestSuite {
        name: "pristine_bootstrap_and_loader",
        test_cases: vec![
            bootstrap_operational_schema_fixture().unwrap(),
            loader_incremental_fixture().unwrap(),
        ],
    }
}

fn runtime_behavior_matrix_suite() -> DanceTestSuite {
    DanceTestSuite {
        name: "runtime_behavior_matrix",
        test_cases: vec![
            load_book_person_inverse_schema_fixture().unwrap(),
            load_inverse_oriented_book_person_instances_fixture().unwrap(),
            stage_new_version_fixture().unwrap(),
            simple_create_holon_fixture().unwrap(),
            commit_validation_fixture().unwrap(),
            commit_no_action_fixture().unwrap(),
            commit_mixed_dispositions_fixture().unwrap(),
            commit_same_key_dispositions_fixture().unwrap(),
            commit_sequential_lineage_fixture().unwrap(),
            commit_non_root_lineage_fixture().unwrap(),
            commit_competition_retry_fixture().unwrap(),
            commit_graph_only_competition_fixture().unwrap(),
            commit_branch_across_transactions_fixture().unwrap(),
            commit_unstaged_schema_finding_fixture().unwrap(),
            commit_schema_cycle_fixture().unwrap(),
            commit_strict_contract_fixture().unwrap(),
            simple_abandon_staged_changes_fixture().unwrap(),
            simple_add_remove_properties_fixture().unwrap(),
            simple_add_remove_related_holons_fixture().unwrap(),
            ergonomic_add_remove_properties_fixture().unwrap(),
            ergonomic_add_remove_related_holons_fixture().unwrap(),
            stage_new_from_clone_fixture().unwrap(),
            transaction_lifecycle_fixture().unwrap(),
            smartlink_commit_cache_fixture().unwrap(),
            delete_holon_fixture().unwrap(),
            frozen_member_head_redirect_fixture().unwrap(),
            frozen_member_head_redirect_cross_tx_fixture().unwrap(),
            cross_transaction_staged_target_diagnostic_fixture().unwrap(),
            query_qry1_scaffold_fixture().unwrap(),
            query_qry2_seed_expand_fixture().unwrap(),
            query_qry4a_order_paginate_fixture().unwrap(),
        ],
    }
}

/// Focused Issue 706 acceptance using the committed shared Book/Person fixture.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "focused artifact export; runtime_behavior_matrix already covers these assertions"]
async fn book_value_presentation_acceptance() {
    run_dance_test_suite(DanceTestSuite {
        name: "book_value_presentation_acceptance",
        test_cases: vec![load_book_person_inverse_schema_fixture().unwrap()],
    })
    .await;
}
