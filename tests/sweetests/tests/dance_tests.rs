//! MAP Dance Test Cases
//!
//! The functions in this file are used in conjunction with Rust rstest test fixtures.
//! inserting holochain_trace::test_run() at the start of a tests driver (e.g., dance_tests)
//! setting RUST_LOG to the desired client-side tracing level to include in output.
//! setting WASM_LOG to the desired guest-side tracing level to include in output.
//! In increasing level of detail:
//! error, warn, info, debug, trace

//! Examples:

//! To show DEBUG level trace messages on the client-side and WARN level trace messages on the guest-side:
//! export RUST_LOG=debug
//! export WASM_LOG=warn

//! To show INFO level trace messages on the client-side and DEBUG level trace messages on the guest-side:
//! export RUST_LOG=info
//! export WASM_LOG=debug

#![allow(unused_imports)]

mod execution_steps;
mod fixture_cases;
mod helpers;

use async_std::prelude::Future;

use holons_core::core_shared_objects::holon;
use rstest::*;
use serde::de::Expected;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, trace, warn, Level};
//use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, reload, registry::Registry};

use execution_steps::abandon_staged_changes_executor::execute_abandon_staged_changes;
use execution_steps::add_related_holons_executor::execute_add_related_holons;
use execution_steps::commit_executor::execute_commit;
use execution_steps::delete_holon_executor::execute_delete_holon;
use execution_steps::ensure_database_count_executor::execute_ensure_database_count;
use execution_steps::load_holons_client_executor::execute_load_holons_client;
use execution_steps::load_holons_executor::execute_load_holons;
use execution_steps::match_db_content_executor::execute_match_db_content;
use execution_steps::new_holon_executor::execute_new_holon;
use execution_steps::query_relationships_executor::execute_query_relationships;
use execution_steps::remove_properties_command_executor::execute_remove_properties;
use execution_steps::remove_related_holon_executor::execute_remove_related_holons;
use execution_steps::stage_new_from_clone_executor::execute_stage_new_from_clone;
use execution_steps::stage_new_holon_executor::execute_stage_new_holon;
use execution_steps::stage_new_version_executor::execute_stage_new_version;
use execution_steps::with_properties_command_executor::execute_with_properties;

use fixture_cases::abandon_staged_changes_fixture::*;
use fixture_cases::delete_holon_fixture::*;
use fixture_cases::ergonomic_add_remove_properties_fixture::*;
use fixture_cases::ergonomic_add_remove_related_holons_fixture::*;
use fixture_cases::load_holons_fixture::*;
use fixture_cases::loader_client_fixture::*;
use fixture_cases::simple_add_remove_properties_fixture::*;
use fixture_cases::simple_add_remove_related_holons_fixture::*;
use fixture_cases::simple_create_holon_fixture::*;
use fixture_cases::stage_new_from_clone_fixture::*;
use fixture_cases::stage_new_version_fixture::*;

use helpers::TEST_CLIENT_PREFIX;
use holons_test::harness::prelude::{DanceTestStep, DancesTestCase};

// use holons_client::init_client_context;
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
// #[case::simple_undescribed_create_holon_test(simple_create_holon_fixture())]
// #[case::delete_holon(delete_holon_fixture())]
// #[case::simple_abandon_staged_changes_test(simple_abandon_staged_changes_fixture())]
// #[case::simple_add_remove_properties_test(simple_add_remove_properties_fixture())]
// #[case::simple_add_related_holon_test(simple_add_remove_related_holons_fixture())]
// #[case::ergonomic_add_remove_properties_test(ergonomic_add_remove_properties_fixture())]
// #[case::ergonomic_add_remove_related_holons_test(ergonomic_add_remove_related_holons_fixture())]
// #[case::stage_new_from_clone_test(stage_new_from_clone_fixture())]
#[case::stage_new_version_test(stage_new_version_fixture())]
// #[case::load_holons_test(loader_incremental_fixture())]
// #[case::load_holons_client_test(loader_client_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_tests(#[case] input: Result<DancesTestCase, HolonError>) {
    // Setup

    // The heavy lifting for this test is in the test data set creation.

    use holons_test::execution_state::TestExecutionState;

    use self::helpers::init_test_context;

    let mut test_case: DancesTestCase = input.unwrap();
    // Initialize test context and execution state
    let test_context = init_test_context(&mut test_case).await;
    let mut test_execution_state = TestExecutionState::new(test_context);

    tracing::info!("Hello from the test!");

    let steps = test_case.clone().steps;
    let name = test_case.clone().name.clone();
    let description = test_case.clone().description;

    let steps_count = steps.len();

    info!("\n\n{TEST_CLIENT_PREFIX} ******* STARTING {name} TEST CASE WITH {steps_count} TEST STEPS ***************************");
    info!("\n   Test Case Description: {description}");

    for step in test_case.steps {
        //println!("\n\n============= STARTING NEXT STEP: {}", step);

        use self::execution_steps::execute_print_database;
        match step {
            DanceTestStep::AbandonStagedChanges {
                source_token,
                expected_token,
                expected_status,
            } => {
                execute_abandon_staged_changes(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    expected_status,
                )
                .await
            }
            DanceTestStep::AddRelatedHolons {
                source_token,
                expected_token,
                relationship_name,
                holons_to_add,
                expected_status,
            } => {
                execute_add_related_holons(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    relationship_name,
                    holons_to_add,
                    expected_status,
                )
                .await
            }
            DanceTestStep::Commit { saved_tokens, expected_status } => {
                execute_commit(&mut test_execution_state, saved_tokens, expected_status).await
            }
            DanceTestStep::DeleteHolon { source_token, expected_token, expected_status } => {
                execute_delete_holon(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    expected_status,
                )
                .await
            }
            DanceTestStep::EnsureDatabaseCount { expected_count } => {
                execute_ensure_database_count(&mut test_execution_state, expected_count).await
            }
            DanceTestStep::LoadHolons {
                set,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
            } => {
                execute_load_holons(
                    &mut test_execution_state,
                    set,
                    expect_staged,
                    expect_committed,
                    expect_links_created,
                    expect_errors,
                    expect_total_bundles,
                    expect_total_loader_holons,
                )
                .await
            }
            DanceTestStep::LoadHolonsClient {
                content_set,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
            } => {
                execute_load_holons_client(
                    &mut test_execution_state,
                    content_set,
                    expect_staged,
                    expect_committed,
                    expect_links_created,
                    expect_errors,
                    expect_total_bundles,
                    expect_total_loader_holons,
                )
                .await
            }
            DanceTestStep::MatchSavedContent => {
                execute_match_db_content(&mut test_execution_state).await
            }
            DanceTestStep::NewHolon { source_token, properties, key, expected_status } => {
                execute_new_holon(
                    &mut test_execution_state,
                    source_token,
                    properties,
                    key,
                    expected_status,
                )
                .await
            }
            DanceTestStep::PrintDatabase => execute_print_database(&mut test_execution_state).await,
            DanceTestStep::QueryRelationships {
                source_token,
                query_expression,
                expected_status,
            } => {
                execute_query_relationships(
                    &mut test_execution_state,
                    source_token,
                    query_expression,
                    expected_status,
                )
                .await
            }
            DanceTestStep::RemoveProperties {
                source_token,
                expected_token,
                properties,
                expected_status,
            } => {
                execute_remove_properties(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    properties,
                    expected_status,
                )
                .await
            }
            DanceTestStep::RemoveRelatedHolons {
                source_token,
                expected_token,
                relationship_name,
                holons_to_remove,
                expected_status,
            } => {
                execute_remove_related_holons(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    relationship_name,
                    holons_to_remove,
                    expected_status,
                )
                .await
            }
            DanceTestStep::StageHolon { source_token, expected_token, expected_status } => {
                execute_stage_new_holon(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    expected_status,
                )
                .await
            }
            DanceTestStep::StageNewFromClone {
                source_token,
                expected_token,
                new_key,
                expected_status,
            } => {
                execute_stage_new_from_clone(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    new_key,
                    expected_status,
                )
                .await
            }
            DanceTestStep::StageNewVersion { source_token, expected_token, expected_status } => {
                execute_stage_new_version(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    expected_status,
                )
                .await
            }
            DanceTestStep::WithProperties {
                source_token,
                expected_token,
                properties,
                expected_status,
            } => {
                execute_with_properties(
                    &mut test_execution_state,
                    source_token,
                    expected_token,
                    properties,
                    expected_status,
                )
                .await
            }
        }
    }
    info!("\n{{TEST_CLIENT_PREFIX}} ------- END OF {name} TEST CASE  ---------------");
}
