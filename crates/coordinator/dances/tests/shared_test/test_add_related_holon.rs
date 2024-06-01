//! Holon Descriptor Test Cases

#![allow(unused_imports)]



use std::collections::BTreeMap;

use async_std::task;
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;
use dances::dance_request::PortableReference;
use dances::holon_dance_adapter::{build_get_all_holons_dance_request, build_add_related_holons_dance_request, build_with_properties_dance_request};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::dance_response::ResponseBody::{Holons, Index};
use holons::commit_manager::StagedIndex;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use crate::shared_test::dance_fixtures::*;
use crate::shared_test::test_data_types::{DancesTestCase, DanceTestState};
use crate::shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use crate::shared_test::test_data_types::DanceTestStep;

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holons
///

pub async fn execute_add_related_holons(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    source_holon_index: StagedIndex,
    relationship_name: RelationshipName,
    holons_to_add: Vec<PortableReference>,
    expected_response: ResponseStatusCode,
) ->() {

    info!("\n\n--- TEST STEP: Adding Related Holons. Expecting  {:#?}", expected_response.clone());


    // Get the state of the holon prior to dancing the request
    let source_holon = test_state.staging_area.staged_holons.get(source_holon_index);
    match source_holon {
        None => {
            panic!("Unable to get source holon from the staging_area at index  {:#?}", source_holon_index.to_string());
        }
        Some(_original_holon) => {
            // Create the expected_holon from the source_holon + the supplied related holons
            // let mut expected_holon = original_holon.clone();


            // Build the DanceRequest
            let request = build_add_related_holons_dance_request(
                test_state.staging_area.clone(),

                source_holon_index,
                relationship_name,
                holons_to_add);
            info!("Dance Request: {:#?}", request);

            match request {
                Ok(valid_request) => {
                    let response: DanceResponse = conductor
                        .call(&cell.zome("dances"), "dance", valid_request)
                        .await;
                    info!("Dance Response: {:#?}", response.clone());
                    let code = response.status_code;

                    test_state.staging_area = response.staging_area.clone();

                    assert_eq!(code, expected_response);
                    info!("as expected, add_related_holons dance request returned {:#?}", code.clone());


                    if let ResponseStatusCode::OK = code {
                        if let Index(index) = response.body {
                            let index_value = index.to_string();
                            info!("{index_value} returned in body");
                            // An index was returned in the body, retrieve the Holon at that index within
                            // the StagingArea and confirm it matches the expected Holon.

                            //let holons = response.staging_area.staged_holons;
                            //assert_eq!(expected_holon, holons[index]);


                            info!("Success! Related Holons have been added");
                        }
                    }
                }
                Err(error) => {
                    panic!("{:?} Unable to build a stage_holon request ", error);
                }
            }
        }

        // // Build a add_related_holons DanceRequest
        // let request = build_add_related_holons_dance_request(
        //     test_state.staging_area.clone(),
        //     source_holon_index,
        //     relationship_name.clone(),
        //     holons_to_add,
        // );
        // println!("Dance Request: {:#?}", request);
        //
        // match request {
        //     Ok(valid_request)=> {
        //
        //         // Ask the Conductor to dance
        //         let response: DanceResponse = conductor
        //             .call(&cell.zome("dances"), "dance", valid_request)
        //             .await;
        //
        //         println!("Dance Response: {:#?}", response.clone());
        //
        //
        //         let code = response.status_code;
        //         // let description = response.description.clone();
        //
        //         // Update test_state with staging_area returned in response
        //         test_state.staging_area = response.staging_area.clone();
        //
        //         // Determine if test succeeded
        //         if let ResponseStatusCode::OK = code {
        //             if let Index(index) = response.body {
        //
        //                 // An index was returned in the body, retrieve the Holon at that index within
        //                 // the StagingArea and confirm it matches the expected Holon.
        //
        //                 let holons = response.staging_area.staged_holons;
        //                 assert_eq!(expected_holon, holons[index]);
        //
        //
        //                 println!("Success! Related Holons have been staged, as expected");
        //             } else {
        //                 panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
        //             }
        //         } else {
        //             panic!("DanceRequest returned {code} for {description}");
        //         }
        //     }
        //     Err(error)=> {
        //         panic!("{:?} Unable to build an add_related_holons dance request. ", error);
        //     }
        // }
    }
}
