//! Holon Descriptor Test Cases

#![allow(unused_imports)]

// use futures::future;
use std::collections::BTreeMap;

mod shared_test;

use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use async_std::task;

use rstest::*;
use shared_test::descriptor_fixtures::*;
use shared_test::*;
// use shared_test::test;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;

use shared_test::test_data_types::{DescriptorTestCase, DescriptorTestStep};
use shared_types_holon::holon_node::{PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;

/// This function iterates through the Vec of Holons provided by the test fixture
///
/// Test Outline:
/// 1. After initial setup, perform a `get_all_holons`, with an expectation of an empty result
/// 2. For each test_holon in the `holons` vector,
///      * create a new holon (to serve as builder)
///      * iterate through the test_holon's properties, invoking external app_property_value for each.
///      * commit the holon
///      * check that the committed holon matches the test_holon
/// /// 3. Once all data has been created in DHT, perform `get_all_holons` and verify the result.
///
/// Note that this will exercise, create, get, and get_all capabilities across a variety of holons
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p descriptors --test descriptor_tests  -- --show-output
///
#[rstest]
#[case::schema_slice_creation(descriptors_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_schema_loading(#[case] input: Result<DescriptorTestCase, HolonError>) {
    // Setup

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // For now, the fixture is being ignored and we simply try to load the schema in one shot
    println!("******* STARTING CORE SCHEMA LOAD ***************************");

    println!("Performing get_all_holons here to ensure initial DB state is empty...");
    // let dummy = String::from("dummy");
    let fetched_holons: Vec<Holon> = conductor
        .call(&cell.zome("holons"), "get_all_holons", ())
        .await;
    assert_eq!(0, fetched_holons.len());

    println!("Success! Initial DB state has no Holons");

    println!("SKIPPING core schema load...");
    // let load_schema_result : Holon = conductor
    //     .call(&cell.zome("descriptors"), "load_core_schema_api", ())
    //     .await;
    // // assert_eq!(1, fetched_holons.len());
    // println!("Call to load_core_schema_api returned: ");
    // println!("{:#?}",load_schema_result);

    // The heavy lifting for this test is in the test data set creation.

    // let mut test_steps: Vec<DescriptorTestStep> = input.unwrap().steps;
    // let step_count = test_steps.len();
    //
    // println!("******* STARTING TESTS WITH {step_count} STEPS ***************************");
    //
    // println!("Performing get_all_holons here to ensure initial DB state is empty...");
    // // let dummy = String::from("dummy");
    // let fetched_holons: Vec<Holon> = conductor
    //     .call(&cell.zome("holons"), "get_all_holons", ())
    //     .await;
    // assert_eq!(0, fetched_holons.len());
    //
    // println!("Success! Initial DB state has no Holons");
    //
    // let mut created_action_hashes: Vec<ActionHash> = Vec::new();
    //
    // // Iterate through the vector of test holons, building & creating each holon,
    // // then get the created holon and compare it to the generated descriptor.
    // for test_step in test_steps.clone() {
    //     match test_step {
    //         DescriptorTestStep::Create(holon_to_create) => {
    //             let created_holon: Holon = conductor
    //                 .call(&cell.zome("holons"), "commit", holon_to_create.clone())
    //                 .await;
    //             let action_hash: ActionHash = created_holon.get_id();
    //             created_action_hashes.push(action_hash.clone());
    //             println!("Fetching created holon");
    //             let fetched_holon: Holon = conductor
    //                 .call(&cell.zome("holons"), "get_holon", action_hash)
    //                 .await;
    //
    //             assert_eq!(holon_to_create.into_node(), fetched_holon.clone().into_node());
    //
    //             println!("\n...Success! Fetched holon matches generated holon ******");
    //             trace!("{:#?}", fetched_holon);
    //         }
    //         DescriptorTestStep::Update(holon_to_update) => {
    //             let updated_holon: Holon = conductor
    //                 .call(&cell.zome("holons"), "commit", holon_to_update.clone())
    //                 .await;
    //             let action_hash: ActionHash = updated_holon.get_id();
    //             created_action_hashes.push(action_hash.clone());
    //             println!("Fetching updated holon");
    //             let fetched_holon: Holon = conductor
    //                 .call(&cell.zome("holons"), "get_holon", action_hash)
    //                 .await;
    //
    //             assert_eq!(holon_to_update.into_node(), fetched_holon.clone().into_node());
    //
    //             println!("\n...Success! Fetched holon matches generated holon ******");
    //             trace!("{:#?}", fetched_holon);
    //         }
    //         DescriptorTestStep::Delete(holon_id_to_delete) =>{
    //             // TODO: figure out to deletes since fixture won't actually have the HolonId of the holon delete
    //         }
    //     }
    // }

    println!("All Steps Completed...");
    println!("To re-run just this test with output, use: 'cargo test -p descriptors --test descriptor_tests  -- --show-output'");
}
fn print_holon_without_saved_node(holon: &Holon) {
    println!("{:#?} Holon: with property map: ", holon.state.clone());
    println!("{:#?}", holon.property_map.clone());
}
