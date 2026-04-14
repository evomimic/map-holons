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

use rstest::*;

use tracing::{
    // error,
    info,
    // trace,
    // warn,
    // Level
};

use execution_steps::abandon_staged_changes_executor::execute_abandon_staged_changes;
use execution_steps::add_related_holons_executor::execute_add_related_holons;
use execution_steps::begin_transaction_executor::execute_begin_transaction;
use execution_steps::commit_executor::execute_commit;
use execution_steps::delete_holon_executor::execute_delete_holon;
use execution_steps::ensure_database_count_executor::execute_ensure_database_count;
use execution_steps::load_core_schema_executor::execute_load_core_schema;
use execution_steps::load_holons_executor::execute_load_holons;
use execution_steps::match_db_content_executor::execute_match_db_content;
use execution_steps::new_holon_executor::execute_new_holon;
use execution_steps::query_relationships_executor::execute_query_relationships;
use execution_steps::remove_properties_executor::execute_remove_properties;
use execution_steps::remove_related_holon_executor::execute_remove_related_holons;
use execution_steps::stage_new_from_clone_executor::execute_stage_new_from_clone;
use execution_steps::stage_new_holon_executor::execute_stage_new_holon;
use execution_steps::stage_new_version_executor::execute_stage_new_version;
use execution_steps::with_properties_executor::execute_with_properties;

use fixture_cases::abandon_staged_changes_fixture::*;
use fixture_cases::delete_holon_fixture::*;
use fixture_cases::ergonomic_add_remove_properties_fixture::*;
use fixture_cases::ergonomic_add_remove_related_holons_fixture::*;
use fixture_cases::load_core_schema_fixture::*;
use fixture_cases::load_holons_fixture::*;
use fixture_cases::simple_add_remove_properties_fixture::*;
use fixture_cases::simple_add_remove_related_holons_fixture::*;
use fixture_cases::simple_create_holon_fixture::*;
use fixture_cases::stage_new_from_clone_fixture::*;
use fixture_cases::stage_new_version_fixture::*;
use fixture_cases::transaction_lifecycle_fixture::*;

use self::execution_steps::execute_print_database;
use holons_test::execution_state::TestExecutionState;
use holons_test::harness::helpers::TEST_CLIENT_PREFIX;
use holons_test::harness::prelude::{DanceTestStep, DancesTestCase};

use holons_test::harness::helpers::init_test_runtime;

use holons_prelude::prelude::*;

/// This function accepts a DanceTestCase created by the test fixture for that case.
/// It iterates through the vector of DanceTestSteps defined within that DanceTestCase.
/// For each step, this function invokes the test execution functions created for that kind of
/// DanceTestStep.
///
/// Prior to initiating the test case, the following initialization is performed:
/// 1. Set up a mock Conductor
/// 2. Initialize a ClientHolonsContext, injecting the ConductorConfig for the created Conductor
///
/// This function maintains the following TestState that allows the test steps to be linked together.
/// * the Context's Nursery will hold the Holons staged during the course of the test case
/// * session_state : SessionState
/// * created_holons -- a BTree of Holons indexed by their key that is incrementally extended as
/// staged holons are committed.
/// It can be used to drive update/delete of those holons.
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p dances --test dance_tests
///      set RUST_LOG to enable client-side (i.e., test code) tracing
///      set WASM_LOG to enable guest-side (i.e., zome code) tracing
///
#[rstest]
#[case::simple_undescribed_create_holon_test(simple_create_holon_fixture())]
#[case::delete_holon(delete_holon_fixture())]
#[case::simple_abandon_staged_changes_test(simple_abandon_staged_changes_fixture())]
#[case::simple_add_remove_properties_test(simple_add_remove_properties_fixture())]
#[case::simple_add_related_holon_test(simple_add_remove_related_holons_fixture())]
#[case::ergonomic_add_remove_properties_test(ergonomic_add_remove_properties_fixture())]
#[case::ergonomic_add_remove_related_holons_test(ergonomic_add_remove_related_holons_fixture())]
#[case::stage_new_from_clone_test(stage_new_from_clone_fixture())]
#[case::stage_new_version_test(stage_new_version_fixture())]
#[case::load_holons_test(loader_incremental_fixture())]
#[case::load_core_schema_test(load_core_schema_fixture())]
#[case::transaction_lifecycle_test(transaction_lifecycle_fixture())]
#[tokio::test(flavor = "multi_thread")]
// TODO: Support for relationships to be finished in issue 382
async fn rstest_dance_tests(#[case] input: Result<DancesTestCase, HolonError>) {
    // Setup

    // The heavy lifting for this test is in the test data set creation.

    let mut test_case: DancesTestCase = input.unwrap();
    assert!(
        test_case.is_finalized(),
        "DancesTestCase must be finalized before execution. Call test_case.finalize(&fixture_context) in the fixture."
    );
    // Initialize runtime and execution state
    let fixture_transient_holons = test_case.test_session_state.get_transient_holons().clone();
    let (runtime, tx_id) = init_test_runtime(&mut test_case).await;
    let mut test_execution_state =
        TestExecutionState::new(runtime, tx_id, fixture_transient_holons);

    info!("\n\n{TEST_CLIENT_PREFIX} ******* STARTING {} TEST CASE WITH {} TEST STEPS ***************************", test_case.name, test_case.steps.len());
    info!("\n   Test Case Description: {}", test_case.description);

    info!("Planned Steps:");
    for (i, step) in test_case.steps.iter().enumerate() {
        info!(" {}. {}", i + 1, step);
    }

    for step in test_case.steps {
        info!("========== STARTING STEP: {}", step);

        match step {
            DanceTestStep::AbandonStagedChanges { step_token, expected_error, .. } => {
                execute_abandon_staged_changes(
                    &mut test_execution_state,
                    step_token,
                    expected_error,
                )
                .await
            }
            DanceTestStep::AddRelatedHolons {
                step_token,
                relationship_name,
                holons_to_add,
                expected_error,
                ..
            } => {
                execute_add_related_holons(
                    &mut test_execution_state,
                    step_token,
                    relationship_name,
                    holons_to_add,
                    expected_error,
                )
                .await
            }
            DanceTestStep::BeginTransaction { expected_error, .. } => {
                execute_begin_transaction(&mut test_execution_state, expected_error).await
            }
            DanceTestStep::Commit { saved_tokens, expected_error, .. } => {
                execute_commit(&mut test_execution_state, saved_tokens, expected_error).await
            }
            DanceTestStep::DeleteHolon { step_token, expected_error, .. } => {
                execute_delete_holon(&mut test_execution_state, step_token, expected_error).await
            }
            DanceTestStep::EnsureDatabaseCount { expected_count, .. } => {
                execute_ensure_database_count(&mut test_execution_state, expected_count).await
            }
            DanceTestStep::LoadHolons {
                set_id,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
            } => {
                execute_load_holons(
                    &mut test_execution_state,
                    set_id,
                    expect_staged,
                    expect_committed,
                    expect_links_created,
                    expect_errors,
                    expect_total_bundles,
                    expect_total_loader_holons,
                )
                .await
            }
            DanceTestStep::LoadCoreSchema { .. } => {
                execute_load_core_schema(&mut test_execution_state).await
            }
            DanceTestStep::MatchSavedContent => {
                execute_match_db_content(&mut test_execution_state).await
            }
            DanceTestStep::NewHolon { step_token, properties, key, expected_error, .. } => {
                execute_new_holon(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    key,
                    expected_error,
                )
                .await
            }
            DanceTestStep::PrintDatabase => execute_print_database(&mut test_execution_state).await,
            DanceTestStep::QueryRelationships {
                step_token,
                query_expression,
                expected_error,
                ..
            } => {
                execute_query_relationships(
                    &mut test_execution_state,
                    step_token,
                    query_expression,
                    expected_error,
                )
                .await
            }
            DanceTestStep::RemoveProperties { step_token, properties, expected_error, .. } => {
                execute_remove_properties(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    expected_error,
                )
                .await
            }
            DanceTestStep::RemoveRelatedHolons {
                step_token,
                relationship_name,
                holons_to_remove,
                expected_error,
                ..
            } => {
                execute_remove_related_holons(
                    &mut test_execution_state,
                    step_token,
                    relationship_name,
                    holons_to_remove,
                    expected_error,
                )
                .await
            }
            DanceTestStep::StageHolon { step_token, expected_error, .. } => {
                execute_stage_new_holon(&mut test_execution_state, step_token, expected_error).await
            }
            DanceTestStep::StageNewFromClone { step_token, new_key, expected_error, .. } => {
                execute_stage_new_from_clone(
                    &mut test_execution_state,
                    step_token,
                    new_key,
                    expected_error,
                )
                .await
            }
            DanceTestStep::StageNewVersion {
                step_token,
                expected_error,
                version_count,
                expected_staging_error,
                ..
            } => {
                execute_stage_new_version(
                    &mut test_execution_state,
                    step_token,
                    expected_error,
                    version_count,
                    expected_staging_error,
                )
                .await
            }
            DanceTestStep::WithProperties { step_token, properties, expected_error, .. } => {
                execute_with_properties(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    expected_error,
                )
                .await
            }
        }
    }
    info!("\n{TEST_CLIENT_PREFIX} ------- END OF {} TEST CASE  ---------------", test_case.name);
}
