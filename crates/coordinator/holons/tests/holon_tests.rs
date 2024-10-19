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
use shared_test::holon_fixtures::*;
use shared_test::test_data_types::{HolonCreatesTestCase, HolonTestCase};
use shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, LocalId};

/// This function iterates through the Vec of Holons provided by the test fixture
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
#[case::simple_undescribed_holons(undescribed_holons_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_holon_capabilities(#[case] input: Result<HolonCreatesTestCase, HolonError>) {
    // Setup

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // The heavy lifting for this test is in the test data set creation.

    let test_holons: Vec<Holon> = input.unwrap().creates;
    let h_count = test_holons.len();

    println!("******* STARTING TESTS WITH {h_count} HOLONS ***************************");

    println!("Performing get_all_holons here to ensure initial DB state is empty...");
    // let dummy = String::from("dummy");
    let fetched_holons: Vec<Holon> = conductor
        .call(&cell.zome("holons"), "get_all_holons", ())
        .await;
    assert_eq!(0, fetched_holons.len());

    println!("Success! Initial DB state has no Holons");

    let mut created_action_hashes: Vec<LocalId> = Vec::new();

    // Iterate through the vector of test holons, building & creating each holon,
    // then get the created holon and compare it to the generated descriptor.
    for test_holon in test_holons.clone() {
        let p_count = test_holon.property_map.len();

        println!("\n****** Starting create/get test for the following Holon:");
        // print_holon_without_saved_node(&test_holon);

        let mut builder_holon = Holon::new();

        for property_name in test_holon.property_map.keys() {
            let property_value: BaseValue =
                test_holon.property_map.get(property_name).unwrap().clone();
            let input = WithPropertyInput {
                holon: builder_holon.clone(),
                property_name: property_name.clone(),
                value: property_value,
            };

            builder_holon = conductor
                .call(&cell.zome("holons"), "with_property_value", input)
                .await;
        }
    }
    println!("SKIPPING REMAINDER OF TESTS... move them to Dance API tests");
    //     let created_holon: Holon = conductor
    //         .call(&cell.zome("holons"), "commit", builder_holon.clone())
    //         .await;
    //
    //     println!("Commit request returned this holon");
    //     println!("{:#?}", created_holon);
    //
    //     if let Ok(id) = created_holon.get_id() {
    //         created_action_hashes.push(id.clone());
    //
    //         println!("Fetching created holon");
    //         let fetched_holon: Holon = conductor.call(&cell.zome("holons"), "get_holon", id).await;
    //
    //         assert_eq!(test_holon.into_node(), fetched_holon.clone().into_node());
    //
    //         println!("\n...Success! Fetched holon matches generated holon ******");
    //         trace!("{:#?}", fetched_holon);
    //     }
    // }
    //
    // println!("All Holon Descriptors Created... do a get_all_holon_types and compare result with test data...");
    // let fetched_holons: Vec<Holon> = conductor
    //     .call(&cell.zome("holons"), "get_all_holons", ())
    //     .await;
    // assert_eq!(h_count, fetched_holons.clone().len());
    //
    // // TESTING DELETES //
    // println!("\n\n *********** TESTING DELETES *******************\n");
    //
    // for hash in created_action_hashes.clone() {
    //     let _deleted_hash: ActionHash = conductor
    //         .call(&cell.zome("holons"), "delete_holon", hash.clone())
    //         .await;
    // }
    //
    // debug!("Performing get_all_holons here to ensure all holons_integrity have been deleted.\n");
    //
    // let fetched_holons: Vec<Holon> = conductor
    //     .call(&cell.zome("holons"), "get_all_holons", ())
    //     .await;
    //
    // assert_eq!(0, fetched_holons.len());
    // println!("...Success! All holons_integrity have been deleted. \n");
    println!("To re-run just this test with output, use: 'cargo test -p holons --test holon_tests  -- --show-output'");
}

fn print_holon_without_saved_node(holon: &Holon) {
    println!("{:#?} Holon: with property map: ", holon.state.clone());
    println!("{:#?}", holon.property_map.clone());
}
