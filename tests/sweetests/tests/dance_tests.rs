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
mod query_contract;
mod runtime_checks;

use execution_steps::dance_test_runner::{
    boot_dance_test_suite, run_dance_test_suite, run_dance_test_suite_on, DanceTestSuite,
};
use fixture_cases::commit_graph_only_fixture::*;
use fixture_cases::commit_key_uniqueness_fixture::*;
use rstest::*;

use fixture_cases::abandon_staged_changes_fixture::*;
use fixture_cases::attachment_defaults_fixture::*;
use fixture_cases::commit_disposition_fixture::*;
use fixture_cases::commit_lineage_fixture::*;
use fixture_cases::commit_scalar_constraints_fixture::*;
use fixture_cases::commit_schema_fixture::*;
use fixture_cases::commit_sibling_fixture::*;
use fixture_cases::commit_staged_scalar_fixture::*;
use fixture_cases::commit_strict_contract_fixture::*;
use fixture_cases::commit_validation_fixture::*;
use fixture_cases::delete_holon_fixture::*;
use fixture_cases::ergonomic_add_remove_properties_fixture::*;
use fixture_cases::ergonomic_add_remove_related_holons_fixture::*;
use fixture_cases::load_book_person_inverse_schema_fixture::*;
use fixture_cases::load_inverse_oriented_book_person_instances_fixture::*;
use fixture_cases::query_f1_filter_fixture::*;
use fixture_cases::query_f2_equality_fixture::*;
use fixture_cases::query_f3_boolean_fixture::*;
use fixture_cases::query_f4_ordering_fixture::*;
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
/// The runtime matrix also runs the shared-runtime integration checks and is
/// selected by the required ci-rest shard to preserve their CI coverage.
#[rstest]
#[case::runtime_behavior_matrix(runtime_behavior_matrix_suite())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_test_suites(#[case] suite: DanceTestSuite) {
    let (runtime, initial_tx_id) = boot_dance_test_suite().await;
    runtime_checks::run_before_fixtures(&runtime).await;
    run_dance_test_suite_on(&runtime, initial_tx_id, suite).await;
    runtime_checks::run(&runtime).await;
}

/// Focused rerun for the fixture-scoped rejection contract; default coverage is in the matrix.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "focused bounded rejection verification; covered by runtime_behavior_matrix"]
async fn commit_validation_correction_acceptance() {
    run_dance_test_suite(DanceTestSuite {
        name: "commit_validation_correction",
        test_cases: vec![commit_validation_fixture().unwrap()],
    })
    .await;
}

/// Isolates inherited-member rejection from preceding runtime-matrix scenarios.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "focused schema rejection diagnosis; covered by runtime_behavior_matrix"]
async fn cross_schema_dependency_gap_acceptance() {
    run_dance_test_suite(DanceTestSuite {
        name: "cross_schema_dependency_gap",
        test_cases: vec![commit_cross_schema_dependency_gap_fixture().unwrap()],
    })
    .await;
}

/// Isolates historical descriptor membership from preceding matrix scenarios.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "focused membership history diagnosis; covered by runtime_behavior_matrix"]
async fn schema_successor_membership_acceptance() {
    run_dance_test_suite(DanceTestSuite {
        name: "schema_successor_membership",
        test_cases: vec![commit_schema_successor_membership_fixture().unwrap()],
    })
    .await;
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
            commit_scalar_constraints_fixture().unwrap(),
            commit_staged_scalar_fixture().unwrap(),
            attachment_defaults_fixture().unwrap(),
            commit_no_action_fixture().unwrap(),
            commit_mixed_dispositions_fixture().unwrap(),
            commit_same_key_dispositions_fixture().unwrap(),
            commit_sequential_lineage_fixture().unwrap(),
            commit_non_root_lineage_fixture().unwrap(),
            commit_version_siblings_fixture().unwrap(),
            commit_no_op_and_graph_only_siblings_fixture().unwrap(),
            commit_disjoint_graph_additions_fixture().unwrap(),
            commit_historical_key_uniqueness_fixture().unwrap(),
            commit_identical_graph_additions_fixture().unwrap(),
            commit_graph_overlap_rejection_fixture().unwrap(),
            commit_graph_then_version_fixture().unwrap(),
            commit_version_then_graph_fixture().unwrap(),
            commit_branch_across_transactions_fixture().unwrap(),
            commit_cross_schema_dependency_gap_fixture().unwrap(),
            commit_schema_cycle_fixture().unwrap(),
            commit_schema_successor_membership_fixture().unwrap(),
            commit_book_title_exact_references_fixture().unwrap(),
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
        ],
    }
}

/// Small real-storage and adapter contract; exhaustive semantics live in query-layer units.
#[tokio::test(flavor = "multi_thread")]
async fn query_runtime() {
    let (runtime, tx_id) = boot_dance_test_suite().await;
    run_dance_test_suite_on(
        &runtime,
        tx_id,
        DanceTestSuite {
            name: "query_storage_boundaries",
            test_cases: vec![
                query_qry1_scaffold_fixture().unwrap(),
                fixture_cases::query_boundary_fixture::query_boundary_fixture().unwrap(),
            ],
        },
    )
    .await;
    query_contract::run(&runtime).await;
}

/// Exhaustive live-storage matrix for explicit diagnosis; semantic coverage also lives in unit tests.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "extended semantic matrix; query_runtime covers storage and adapter contracts"]
async fn query_semantic_matrix() {
    run_dance_test_suite(DanceTestSuite {
        name: "query_runtime",
        test_cases: vec![
            query_qry1_scaffold_fixture().unwrap(),
            query_qry2_seed_expand_fixture().unwrap(),
            query_qry4a_order_paginate_fixture().unwrap(),
            query_f1_filter_fixture().unwrap(),
            query_f2_equality_fixture().unwrap(),
            query_f3_boolean_fixture().unwrap(),
            query_f4_ordering_fixture().unwrap(),
        ],
    })
    .await;
}

/// Select one existing query fixture after a failure without rerunning its siblings.
/// These diagnostic selectors add no work to the normal dance suite.
#[rstest]
#[case::scaffold(query_qry1_scaffold_fixture)]
#[case::seed_expand(query_qry2_seed_expand_fixture)]
#[case::order_paginate(query_qry4a_order_paginate_fixture)]
#[case::filter(query_f1_filter_fixture)]
#[case::equality(query_f2_equality_fixture)]
#[case::boolean(query_f3_boolean_fixture)]
#[case::ordering(query_f4_ordering_fixture)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "focused extended diagnosis; query_semantic_matrix covers all seven fixtures"]
async fn query_fixture(
    #[case] build: fn() -> Result<holons_test::DancesTestCase, core_types::HolonError>,
) {
    run_dance_test_suite(DanceTestSuite {
        name: "query_fixture",
        test_cases: vec![build().unwrap()],
    })
    .await;
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
