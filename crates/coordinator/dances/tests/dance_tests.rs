//! Holon Descriptor Test Cases

#![allow(unused_imports)]

mod shared_test;

use std::collections::BTreeMap;

use async_std::task;
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use shared_test::dance_fixtures::*;
use shared_test::test_data_types::{DancesTestCase};
use shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::HolonId;
use crate::shared_test::test_data_types::DanceTestStep;
use crate::shared_test::ensure_database_count::execute_ensure_database_count;
//use crate::shared_test::ensure_database_count::*;

/// This function iterates through a vector of test steps provided by the test fixture
///
/// Test Outline:
/// 1. After initial setup, perform a `get_all_holons`, with an expectation of an empty result
/// 2. For each test_holon in the `holons` vector,
///      * create a new holon (to serve as builder)
///      * iterate through the test_holon's properties, invoking external app_property_value for each.
///      * commit the holon
///      * check that the committed holon matches the test_holon
/// 3. Once all data has been created in DHT, perform `get_all_holons` and verify the result.
///
/// Note that this will exercise, create, get, and get_all capabilities across a variety of holons
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p holons --test holon_tests  -- --show-output
///
#[rstest]
#[case::simple_undescribed_create_holon_test(simple_create_test_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_tests(#[case] input: Result<DancesTestCase, HolonError>) {
    // Setup

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // The heavy lifting for this test is in the test data set creation.

    let test_case: DancesTestCase = input.unwrap();
    let steps = test_case.clone().steps;
    let name = test_case.clone().name.clone();
    let description = test_case.clone().description;

    let steps_count = steps.len();

    println!("******* STARTING {name} TEST CASE WITH {steps_count} TEST STEPS ***************************");
    println!("******* {description}  ***************************");

    for step in test_case.steps {
        println!("--- executing next test step");
        match step {
            DanceTestStep::EnsureDatabaseCount(expected_count) => execute_ensure_database_count(&conductor, &cell, expected_count).await,
            // DanceTestStep::Create(holon) => execute_create_step(&conductor, &cell, holon),
            // DanceTestStep::Update(holon) => execute_update_step(&conductor, &cell, holon),
            // DanceTestStep::Delete(holon_id) => execute_delete_step(&conductor, &cell, holon_id),
        }
    }
    println!("-------------- END OF {name} TEST CASE  ------------------");
}

