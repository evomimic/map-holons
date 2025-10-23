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

mod shared_test;

use async_std::prelude::Future;
use std::collections::BTreeMap;
use std::rc::Rc;

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
// use holons_client::init_client_context;

use rstest::*;
use serde::de::Expected;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, trace, warn, Level};
//use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, reload, registry::Registry};

use self::test_abandon_staged_changes::execute_abandon_staged_changes;
use self::test_add_related_holon::execute_add_related_holons;
use self::test_commit::execute_commit;
use self::test_ensure_database_count::execute_ensure_database_count;
use self::test_load_holons::execute_load_holons;
use self::test_match_db_content::execute_match_db_content;
use self::test_query_relationships::execute_query_relationships;
use self::test_remove_properties_command::execute_remove_properties;
use self::test_remove_related_holon::execute_remove_related_holons;
use self::test_with_properties_command::execute_with_properties;

use crate::load_holons_fixture::*;
use crate::shared_test::{
    // mock_conductor::*,
    test_context::init_test_context,
    test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase, TEST_CLIENT_PREFIX},
    test_print_database::execute_database_print,
    test_stage_new_holon::execute_stage_new_holon,
};
use crate::stage_new_from_clone_fixture::*;
use crate::stage_new_version_fixture::*;
use holons_client::init_client_context;
use holons_core::dances::DanceCallServiceApi; // temporary import for dance calls
use holons_prelude::prelude::*;
use shared_test::*;

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
#[case::add_remove_properties_test(ergonomic_add_remove_properties_fixture())]
#[case::add_remove_related_holons_test(ergonomic_add_remove_related_holons_fixture())]
#[case::simple_add_related_holon_test(simple_add_remove_related_holons_fixture())]
#[case::simple_stage_new_from_clone_test(simple_stage_new_from_clone_fixture())]
#[case::simple_stage_new_version_test(simple_stage_new_version_fixture())]
// #[case::load_core_schema(load_core_schema_test_fixture())]
#[case::loader_minimal_test(loader_minimal_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_tests(
    #[case] input: impl Future<Output = Result<DancesTestCase, HolonError>>,
) {
    // Setup

    use test_stage_new_from_clone::execute_stage_new_from_clone;
    use test_stage_new_version::execute_stage_new_version;
    // use test_stage_new_version::execute_stage_new_version;

    use test_delete_holon::execute_delete_holon;

    // The heavy lifting for this test is in the test data set creation.

    let mut test_case: DancesTestCase = input.await.unwrap();
    // Initialize TestHolonsContext from test_session_state
    let test_context: Arc<dyn HolonsContextBehavior> = init_test_context(&mut test_case).await;

    tracing::info!("Hello from the test!");

    let steps = test_case.clone().steps;
    let name = test_case.clone().name.clone();
    let description = test_case.clone().description;

    let steps_count = steps.len();

    warn!("TEST SESSION STATE {:?}", test_case.test_session_state);

    // Initialize the DanceTestState
    let mut test_state = DanceTestExecutionState::new(test_context);

    info!("\n\n{TEST_CLIENT_PREFIX} ******* STARTING {name} TEST CASE WITH {steps_count} TEST STEPS ***************************");
    info!("\n   Test Case Description: {description}");

    for step in test_case.steps {
        //println!("\n\n============= STARTING NEXT STEP: {}", step);
        match step {
            DanceTestStep::AbandonStagedChanges(staged_reference, expected_response) => {
                execute_abandon_staged_changes(&mut test_state, staged_reference, expected_response)
                    .await
            }
            DanceTestStep::AddRelatedHolons(
                holon_reference,
                relationship_name,
                holons_to_add,
                expected_response,
                expected_holon,
            ) => {
                execute_add_related_holons(
                    &mut test_state,
                    holon_reference,
                    relationship_name,
                    holons_to_add,
                    expected_response,
                    expected_holon,
                )
                .await
            }
            DanceTestStep::Commit => execute_commit(&mut test_state).await,
            DanceTestStep::DatabasePrint => execute_database_print(&mut test_state).await,
            DanceTestStep::DeleteHolon(holon_to_delete, expected_response) => {
                execute_delete_holon(&mut test_state, holon_to_delete, expected_response).await
            }
            DanceTestStep::EnsureDatabaseCount(expected_count) => {
                execute_ensure_database_count(&mut test_state, expected_count).await
            }
            DanceTestStep::LoadHolons {
                bundle,
                expect_status,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
            } => {
                execute_load_holons(
                    &mut test_state,
                    bundle,
                    expect_status,
                    expect_staged,
                    expect_committed,
                    expect_links_created,
                    expect_errors,
                )
                .await
            }
            DanceTestStep::MatchSavedContent => execute_match_db_content(&mut test_state).await,
            DanceTestStep::QueryRelationships(
                node_collection,
                query_expression,
                expected_response,
            ) => {
                execute_query_relationships(
                    &mut test_state,
                    node_collection,
                    query_expression,
                    expected_response,
                )
                .await
            }
            DanceTestStep::RemoveProperties(holon_reference, properties, expected_response) => {
                execute_remove_properties(
                    &mut test_state,
                    holon_reference,
                    properties,
                    expected_response,
                )
                .await
            }
            DanceTestStep::RemoveRelatedHolons(
                holon_reference,
                relationship_name,
                holons_to_remove,
                expected_response,
            ) => {
                execute_remove_related_holons(
                    &mut test_state,
                    holon_reference,
                    relationship_name,
                    holons_to_remove,
                    expected_response,
                )
                .await
            }

            DanceTestStep::StageHolon(holon) => {
                execute_stage_new_holon(&mut test_state, holon).await
            }
            DanceTestStep::StageNewFromClone(original_holon, new_key, expected_response) => {
                execute_stage_new_from_clone(
                    &mut test_state,
                    original_holon,
                    new_key,
                    expected_response,
                )
                .await
            }
            DanceTestStep::StageNewVersion(original_holon_key, expected_response) => {
                execute_stage_new_version(&mut test_state, original_holon_key, expected_response)
                    .await
            }
            DanceTestStep::WithProperties(holon_reference, properties, expected_response) => {
                execute_with_properties(
                    &mut test_state,
                    holon_reference,
                    properties,
                    expected_response,
                )
                .await
            }
        }
    }
    info!("\n{{TEST_CLIENT_PREFIX}} ------- END OF {name} TEST CASE  ---------------");
}
