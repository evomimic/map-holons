//! Holon Descriptor Test Cases

#![allow(unused_imports)]

mod shared_test;

use std::collections::BTreeMap;

use async_std::task;
use hdk::prelude::*;
use holochain::prelude::kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::holochain_trace;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use dances::staging_area::StagingArea;
use shared_test::dance_fixtures::*;
use shared_test::test_data_types::{DancesTestCase};
use shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::HolonId;
use crate::shared_test::test_add_related_holon::execute_add_related_holons;
use crate::shared_test::test_commit::execute_commit;
use crate::shared_test::test_data_types::{DanceTestState, DanceTestStep};
use crate::shared_test::test_ensure_database_count::execute_ensure_database_count;
use crate::shared_test::test_stage_new_holon::execute_stage_new_holon;
use crate::shared_test::test_with_properties_command::execute_with_properties;
//use crate::shared_test::ensure_database_count::*;

/// This function accepts a DanceTestCase created by the test fixture for that case.
/// It iterates through the vector of DanceTestSteps defined within that DanceTestCase.
/// For each step, this function invokes the test execution functions created for that kind of
/// DanceTestStep.
///
/// This function maintains the following  TestState that allows the test steps to be linked together.
/// * staging_area -- initially set to empty and then reset from the results of each test step
/// * created_holons -- a vector of Holon that is incrementally extended by test steps. It can be used to drive update/delete of those holons.
/// * TBD
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p dances --test dance_tests  -- --show-output
///
#[rstest]
//#[case::simple_undescribed_create_holon_test(simple_create_test_fixture())]
#[case::simple_add_related_holon_test(simple_add_related_holons_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_tests(#[case] input: Result<DancesTestCase, HolonError>) {
    // Setup
    let _ = holochain_trace::test_run().ok();

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // The heavy lifting for this test is in the test data set creation.

    let test_case: DancesTestCase = input.unwrap();
    let steps = test_case.clone().steps;
    let name = test_case.clone().name.clone();
    let description = test_case.clone().description;

    let steps_count = steps.len();

    // Initialize the DanceTestState
    let mut test_state =DanceTestState::new();

    info!("******* STARTING {name} TEST CASE WITH {steps_count} TEST STEPS ***************************");
    info!("******* {description}  ***************************");

    for step in test_case.steps {
        //println!("\n\n============= STARTING NEXT STEP: {}", step);
        match step {
            DanceTestStep::AddRelatedHolons(staged_index, relationship_name,holons_to_add) => execute_add_related_holons(&conductor, &cell, &mut test_state, staged_index, relationship_name, holons_to_add).await,
            DanceTestStep::EnsureDatabaseCount(expected_count) => execute_ensure_database_count(&conductor, &cell, &mut test_state, expected_count).await,
            DanceTestStep::StageHolon(holon) => execute_stage_new_holon(&conductor, &cell, &mut test_state, holon).await,
            DanceTestStep::Commit() => execute_commit(&conductor, &cell, &mut test_state,).await,
            DanceTestStep::WithProperties(staged_index, properties) => execute_with_properties(&conductor, &cell, &mut test_state, staged_index, properties).await,
            // DanceTestStep::Update(holon) => execute_update_step(&conductor, &cell, holon),
            // DanceTestStep::Delete(holon_id) => execute_delete_step(&conductor, &cell, holon_id),
        }
    }
    info!("-------------- END OF {name} TEST CASE  ------------------");
}

