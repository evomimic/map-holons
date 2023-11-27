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
use shared_test::holon_fixtures::*;
use shared_test::*;
// use shared_test::test;
use shared_test::test_data_types::{HolonTestCase, HolonCreatesTestCase};
use holons::holon_errors::HolonError;
use holons::holon::Holon;
use holons::holon_api::*;

use shared_types_holon::holon_node::{PropertyName, PropertyMap, PropertyValue};



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
/// Note that this will exercise, create, get, and get_all capabilities across a variety of
/// holons
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p holons --test holon_tests  -- --show-output
///
#[rstest]
#[case::create_value_descriptor_holon(new_holons_fixture())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_holon_capabilities(
    #[case] input: Result<HolonCreatesTestCase, HolonError>,
) {
    // Setup

    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    // The heavy lifting for this test is in the test data set creation. Rich descriptors can be
    // built in the create_dummy_data fn to test a broad range of data structures

    let mut test_holons: Vec<Holon> = input.unwrap().creates;
    let h_count = test_holons.len();

    println!("******* STARTING TESTS WITH {h_count} HOLONS ***************************");

    // println!("Performing get_all_holons here to ensure initial DB state is empty");
    // //let dummy = String::from("dummy");
    // let fetched_holons : ExternResult<Vec<Holon>> = conductor
    //     .call(
    //         &cell.zome("holons"),
    //         "get_all_holons",
    //         (),
    //     )
    //     .await;
    //
    // match fetched_holons {
    //     Ok(result) => {
    //         assert_eq!(0, result.len());
    //         println!("Success! Initial DB state has no Holons");
    //     }
    //     Err(e) => {
    //         println!("Error: {:#?}", e);
    //         assert!(false);
    //     }
    // }

    let mut created_action_hashes: Vec<ActionHash> = Vec::new();

    // Iterate through the vector of test holons, building & creating each holon,
    // then get the created holon and comparing it to the generated descriptor.
    for test_holon in test_holons.clone() {
        let p_count = test_holon.property_map.len();
        println!();
        println!("****** Starting create/get test for the following Holon");
        println!("{:#?}", test_holon.clone());

        let mut builder_holon = Holon::new();

        for property_name in test_holon.property_map.keys() {
            let property_value = test_holon.property_map.get(property_name).unwrap().clone();
            let input = AddPropertyInput {
                holon: builder_holon.clone(),
                property_name: property_name.clone(),
                value: property_value,
            };
            builder_holon = conductor
                .call(
                    &cell.zome("holons"),
                    "add_property_value",
                    input,
                )
                .await;
        }
        let created_holon: Holon = conductor
            .call(
                &cell.zome("holons"),
                "commit",
                builder_holon.clone(),
            )
            .await;
        let action_hash: ActionHash = created_holon.get_id();
        created_action_hashes.push(action_hash.clone());

        println!("Fetching created holon");
        let fetched_holon: Holon  = conductor
            .call(&cell.zome("holons"), "get_holon", action_hash)
            .await;

        assert_eq!(test_holon.into_node(), fetched_holon.clone().into_node());

        println!("\n...Success! Fetched holon matches generated holon ******");
        println!("{:#?}", fetched_holon);
    }

    // println!("All Holon Descriptors Created... do a get_all_holon_types and compare result with test data...");
    // let fetched_holons : ExternResult<Vec<Holon>> = conductor
    //     .call(
    //         &cell.zome("holons"),
    //         "get_all_holons",
    //         (),
    //     )
    //     .await;
    //
    // match fetched_holons {
    //     Ok(result) => {
    //         assert_eq!(h_count, result.len());
    //         println!("As expected, get_all_holons returned {h_count} Holons");
    //         //fetched_entries.sort_by(|a, b| a.header.type_name.cmp(&b.header.type_name));
    //         //assert_eq!(holons, fetched_entries);
    //
    //     }
    //     Err(e) => {
    //         println!("Error: {:#?}", e);
    //         assert!(false);
    //     }
    // }

    // TESTING DELETES //
    println!("\n\n *********** TESTING DELETES *******************\n");

    for hash in created_action_hashes {
          let _ = Holon::delete_holon(hash);
    }

    // debug!("Performing get_all_holons here to ensure all holons have been deleted.\n");
    //
    // let fetched_holons : ExternResult<Vec<Holon>> = conductor
    //     .call(
    //         &cell.zome("holons"),
    //         "get_all_holons",
    //         (),
    //     )
    //     .await;
    //
    // match fetched_holons {
    //     Ok(result) => {
    //         assert_eq!(0, result.len());
    //         println!("Success! DB has no Holons");
    //     }
    //     Err(e) => {
    //         println!("Error: {:#?}", e);
    //         assert!(false);
    //     }
    // }
}

