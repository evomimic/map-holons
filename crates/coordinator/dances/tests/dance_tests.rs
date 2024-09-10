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

use std::collections::BTreeMap;

use async_std::task;
use hdk::prelude::*;

use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::holochain_trace;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;
use serde::de::Expected;
use std::sync::{Arc, Mutex};
use test_query_relationships::execute_query_relationships;
use tracing::{debug, error, info, trace, warn, Level};
//use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, reload, registry::Registry};

use self::test_abandon_staged_changes::execute_abandon_staged_changes;
use self::test_add_related_holon::execute_add_related_holons;
use self::test_commit::execute_commit;
use self::data_types::{DanceTestState, DanceTestStep};
use self::test_ensure_database_count::execute_ensure_database_count;
use self::test_load_core_schema::execute_load_new_schema;
use self::test_match_db_content::execute_match_db_content;
use self::test_remove_related_holon::execute_remove_related_holons;
use self::test_stage_new_holon::execute_stage_new_holon;
use self::test_with_properties_command::execute_with_properties;
use crate::dance_fixtures::*;
use crate::descriptor_dance_fixtures::*;
use crate::new_version_fixture::*;
use crate::stage_new_from_clone_fixture::*;
use dances::staging_area::StagingArea;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;

use shared_test::data_types::DancesTestCase;
use shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::HolonId;
//use crate::shared_test::ensure_database_count::*;

/// This function accepts a DanceTestCase created by the test fixture for that case.
/// It iterates through the vector of DanceTestSteps defined within that DanceTestCase.
/// For each step, this function invokes the test execution functions created for that kind of
/// DanceTestStep.
///
/// This function maintains the following TestState that allows the test steps to be linked together.
/// * staging_area -- initially set to empty and then reset from the results of each test step
/// * created_holons -- a vector of Holon that is incrementally extended by test steps. It can be used to drive update/delete of those holons.
/// * TBD
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p dances --test dance_tests
///      set RUST_LOG to enable client-side (i.e., test code) tracing
///      set WASM_LOG to enable guest-side (i.e., zome code) tracing
///
#[rstest]
// #[case::simple_undescribed_create_holon_test(simple_create_test_fixture())]
// #[case::simple_add_related_holon_test(simple_add_remove_related_holons_fixture())]
// #[case::simple_abandon_staged_changes_test(simple_abandon_staged_changes_fixture())]
// #[case::load_core_schema(load_core_schema_test_fixture())]
#[case::simple_stage_new_from_clone_test(simple_stage_new_from_clone_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_tests(#[case] input: Result<DancesTestCase, HolonError>) {
    // Setup

    use test_stage_new_from_clone::execute_stage_new_from_clone;
    let _ = holochain_trace::test_run();

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // The heavy lifting for this test is in the test data set creation.

    let test_case: DancesTestCase = input.unwrap();
    let steps = test_case.clone().steps;
    let name = test_case.clone().name.clone();
    let description = test_case.clone().description;

    let steps_count = steps.len();

    // Initialize the DanceTestState
    let mut test_state = DanceTestState::new();

    info!("******* STARTING {name} TEST CASE WITH {steps_count} TEST STEPS ***************************");
    info!("******* {description}  ***************************");

    for step in test_case.steps {
        //println!("\n\n============= STARTING NEXT STEP: {}", step);
        match step {
            DanceTestStep::AbandonStagedChanges(staged_index, expected_response) => {
                execute_abandon_staged_changes(
                    &conductor,
                    &cell,
                    &mut test_state,
                    staged_index,
                    expected_response,
                )
                .await
            }
            DanceTestStep::AddRelatedHolons(
                staged_index,
                relationship_name,
                holons_to_add,
                expected_response,
                expected_holon,
            ) => {
                execute_add_related_holons(
                    &conductor,
                    &cell,
                    &mut test_state,
                    staged_index,
                    relationship_name,
                    holons_to_add,
                    expected_response,
                    expected_holon,
                )
                .await
            }
            DanceTestStep::Commit => execute_commit(&conductor, &cell, &mut test_state).await,
            DanceTestStep::DatabasePrint => {
                execute_commit(&conductor, &cell, &mut test_state).await
            }
            DanceTestStep::EnsureDatabaseCount(expected_count) => {
                execute_ensure_database_count(&conductor, &cell, &mut test_state, expected_count)
                    .await
            }
            DanceTestStep::LoadCoreSchema => {
                execute_load_new_schema(&conductor, &cell, &mut test_state).await
            }
            DanceTestStep::MatchSavedContent => {
                execute_match_db_content(&conductor, &cell, &mut test_state).await
            }
            // DanceTestStep::NewVersion(holon) => {
            //     execute__new_version(&conductor, &cell, &mut test_state, holon).await
            // }
            DanceTestStep::QueryRelationships(
                node_collection,
                query_expression,
                expected_response,
            ) => {
                execute_query_relationships(
                    &conductor,
                    &cell,
                    &mut test_state,
                    node_collection,
                    query_expression,
                    expected_response,
                )
                .await
            }
            DanceTestStep::RemoveRelatedHolons(
                staged_index,
                relationship_name,
                holons_to_remove,
                expected_response,
                expected_holon,
            ) => {
                execute_remove_related_holons(
                    &conductor,
                    &cell,
                    &mut test_state,
                    staged_index,
                    relationship_name,
                    holons_to_remove,
                    expected_response,
                    expected_holon,
                )
                .await
            }
            DanceTestStep::StageHolon(holon) => {
                execute_stage_new_holon(&conductor, &cell, &mut test_state, holon).await
            }
            DanceTestStep::StageNewFromClone(
                holon_reference,
                expected_response,
                expected_holon,
            ) => {
                execute_stage_new_from_clone(
                    &conductor,
                    &cell,
                    &mut test_state,
                    holon_reference,
                    expected_response,
                    expected_holon,
                )
                .await
            }
            DanceTestStep::WithProperties(staged_index, properties, expected_response) => {
                execute_with_properties(
                    &conductor,
                    &cell,
                    &mut test_state,
                    staged_index,
                    properties,
                    expected_response,
                )
                .await
            }
        }
    }
    info!("-------------- END OF {name} TEST CASE  ------------------");
}
