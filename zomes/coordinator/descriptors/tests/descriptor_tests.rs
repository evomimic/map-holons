//! Holon Descriptor Test Cases
//
// #![allow(unused_imports)]
//
// // use futures::future;
// use std::collections::BTreeMap;
//
// mod shared_test;
//
// use hdk::prelude::*;
// use holochain::sweettest::*;
// use holochain::sweettest::{SweetCell, SweetConductor};
//
// use async_std::task;
// use holochain::prelude::kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::holochain_trace;
//
// use rstest::*;
// use dances::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
// use dances::holon_dance_adapter::build_commit_dance_request;
// use dances::staging_area::StagingArea;
// use descriptors::loader::load_core_schema;
// use holons::cache_manager::HolonCacheManager;
// use holons::commit_manager::{CommitManager, CommitRequestStatus};
// use holons::commit_manager::CommitRequestStatus::Incomplete;
// use holons::context::HolonsContext;
// use shared_test::descriptor_fixtures::*;
// use shared_test::*;
// // use shared_test::test;
// use holons::holon::Holon;
// use holons::holon_api::*;
// use holons::holon_error::HolonError;
//
// use shared_test::test_data_types::{DescriptorTestCase, DescriptorTestStep};
// use shared_types_holon::holon_node::{PropertyMap, PropertyName};
// use shared_types_holon::MapInteger;
// use shared_types_holon::value_types::BaseValue;
// use crate::shared_test::test_data_types::DescriptorTestState;
//
// /// This function currently just invokes the load_core_schema dance. Later we can add incremental
// /// schema evolution dances based on information constructed in the test fixture.
// ///
// /// To selectively run JUST THE TESTS in this file, use:
// ///      cargo test -p descriptors --test descriptor_tests  --
// ///
// #[rstest]
// #[case::schema_slice_creation(descriptors_fixture())]
// #[tokio::test(flavor = "multi_thread")]
// async fn rstest_schema_loading(#[case] input: Result<DescriptorTestCase, HolonError>) {
//
//     // Setup
//     let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
//         setup_conductor().await;
//
//     let _ = holochain_trace::test_run().ok();
//
//     // Initialize the DanceTestState
//     let mut test_state =DescriptorTestState::new();
//
//     // Initialize the context
//     let context = HolonsContext::init_context(CommitManager::new(), HolonCacheManager::new());
//
//     // For now, the fixture is being ignored and we simply try to load the schema in one shot
//     info!("******* TEST SETUP COMPLETE: STARTING CORE SCHEMA LOAD ***************************");
//
//     // // Stage schema with descriptors... then commit
//     // let loaded_schema = load_core_schema(&context);
//     //
//     // match loaded_schema {
//     //     Ok(schema_ref) => {
//     //         info!("*** SCHEMA LOADED SUCCESSFULLY... preparing to commit");
//     //     }
//     //     Err(e)=> {
//     //         panic!("{:?} Unable to build a stage_holon request ", e);
//     //
//     //     }
//     // }
//     //
//     // info!("\n\n--- TEST STEP: Committing Staged Holons ---- :");
//
//
//     // Build a load_core_schema DanceRequest
//     // // Initialize staging_area from the commit_manager
//     // let commit_manager = context.commit_manager.borrow();
//     // test_state.staging_area= StagingArea::from_commit_manager(&commit_manager);
//
//
//     let request = build_load_core_schema_dance_request(test_state.staging_area);
//
//     debug!("Dance Request: {:#?}", request);
//
//     match request {
//         Ok(valid_request) => {
//             let response: DanceResponse = conductor
//                 .call(&cell.zome("dances"), "dance", valid_request)
//                 .await;
//
//             debug!("Dance Response: {:#?}", response.clone());
//             test_state.staging_area = response.staging_area.clone();
//             let code = response.status_code;
//             let description = response.description.clone();
//             if code == ResponseStatusCode::OK {
//                 // Check that staging area is empty
//                 assert!(response.staging_area.staged_holons.is_empty());
//
//                 info!("Success! Commit succeeded");
//
//                 // get saved holons out of response body and add them to the test_state created holons
//                 match response.body {
//                     ResponseBody::Holon(holon) => {
//                         test_state.created_holons.push(holon);
//                     }
//                     ResponseBody::Holons(holons) => {
//                         for holon in holons {
//                             test_state.created_holons.push(holon);
//                         }
//                     }
//                     _ => panic!("Invalid ResponseBody: {:?}", response.body),
//                 }
//
//             } else {
//                 panic!("DanceRequest returned {code} for {description}");
//             }
//         }
//         Err(error) => {
//             panic!("{:?} Unable to build a stage_holon request ", error);
//         }
//     }
//
//     // println!("Performing get_all_holons here to ensure initial DB state is empty...");
//     // // let dummy = String::from("dummy");
//     // let fetched_holons: Vec<Holon> = conductor
//     //     .call(&cell.zome("holons"), "get_all_holons", ())
//     //     .await;
//     // assert_eq!(0, fetched_holons.len());
//     //
//     // println!("Success! Initial DB state has no Holons");
//     //
//     // println!("SKIPPING core schema load...");
//     // let load_schema_result : Holon = conductor
//     //     .call(&cell.zome("descriptors"), "load_core_schema_api", ())
//     //     .await;
//     // // assert_eq!(1, fetched_holons.len());
//     // println!("Call to load_core_schema_api returned: ");
//     // println!("{:#?}",load_schema_result);
//
//     // The heavy lifting for this test is in the test data set creation.
//
//     // let mut test_steps: Vec<DescriptorTestStep> = input.unwrap().steps;
//     // let step_count = test_steps.len();
//     //
//     // println!("******* STARTING TESTS WITH {step_count} STEPS ***************************");
//     //
//     // println!("Performing get_all_holons here to ensure initial DB state is empty...");
//     // // let dummy = String::from("dummy");
//     // let fetched_holons: Vec<Holon> = conductor
//     //     .call(&cell.zome("holons"), "get_all_holons", ())
//     //     .await;
//     // assert_eq!(0, fetched_holons.len());
//     //
//     // println!("Success! Initial DB state has no Holons");
//     //
//     // let mut created_action_hashes: Vec<ActionHash> = Vec::new();
//     //
//     // // Iterate through the vector of test holons, building & creating each holon,
//     // // then get the created holon and compare it to the generated descriptor.
//     // for test_step in test_steps.clone() {
//     //     match test_step {
//     //         DescriptorTestStep::Create(holon_to_create) => {
//     //             let created_holon: Holon = conductor
//     //                 .call(&cell.zome("holons"), "commit", holon_to_create.clone())
//     //                 .await;
//     //             let action_hash: ActionHash = created_holon.get_id();
//     //             created_action_hashes.push(action_hash.clone());
//     //             println!("Fetching created holon");
//     //             let fetched_holon: Holon = conductor
//     //                 .call(&cell.zome("holons"), "get_holon", action_hash)
//     //                 .await;
//     //
//     //             assert_eq!(holon_to_create.into_node(), fetched_holon.clone().into_node());
//     //
//     //             println!("\n...Success! Fetched holon matches generated holon ******");
//     //             trace!("{:#?}", fetched_holon);
//     //         }
//     //         DescriptorTestStep::Update(holon_to_update) => {
//     //             let updated_holon: Holon = conductor
//     //                 .call(&cell.zome("holons"), "commit", holon_to_update.clone())
//     //                 .await;
//     //             let action_hash: ActionHash = updated_holon.get_id();
//     //             created_action_hashes.push(action_hash.clone());
//     //             println!("Fetching updated holon");
//     //             let fetched_holon: Holon = conductor
//     //                 .call(&cell.zome("holons"), "get_holon", action_hash)
//     //                 .await;
//     //
//     //             assert_eq!(holon_to_update.into_node(), fetched_holon.clone().into_node());
//     //
//     //             println!("\n...Success! Fetched holon matches generated holon ******");
//     //             trace!("{:#?}", fetched_holon);
//     //         }
//     //         DescriptorTestStep::Delete(holon_id_to_delete) =>{
//     //             // TODO: figure out to deletes since fixture won't actually have the HolonId of the holon delete
//     //         }
//     //     }
//     // }
//
//     info!("All Steps Completed...");
//     info!("To re-run just this test with output, use: 'cargo test -p descriptors --test descriptor_tests'");
// }
// fn print_holon_without_saved_node(holon: &Holon) {
//     println!("{:#?} Holon: with property map: ", holon.state.clone());
//     println!("{:#?}", holon.property_map.clone());
// }
