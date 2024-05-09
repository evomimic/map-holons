//! Holon Descriptor Test Cases

#![allow(unused_imports)]



use std::collections::BTreeMap;

use async_std::task;
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use rstest::*;
use dances::holon_dance_adapter::{build_get_all_holons_dance_request, build_stage_new_holon_dance_request};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::dance_response::ResponseBody::Holons;

use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use crate::shared_test::dance_fixtures::*;
use crate::shared_test::test_data_types::{DancesTestCase, DanceTestState};
use crate::shared_test::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use crate::shared_test::test_data_types::DanceTestStep;

/// This function builds and dances a `get_all_holons` DanceRequest and confirms that the number
/// of holons returned matches the expected_count of holons provided.
///

pub async fn execute_ensure_database_count(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    expected_count: MapInteger
){
    let expected_count_string = expected_count.0.to_string();
    println!("--- Ensuring database holds {expected_count_string} holons ---");
    // Build a get_all_holons DanceRequest
    let request = build_get_all_holons_dance_request(test_state.staging_area.clone());
    println!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request)=> {
            let response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            test_state.staging_area = response.staging_area.clone();
            if let Some(body) = response.body {
                if let Holons(holons) = body {
                    assert_eq!(expected_count, MapInteger(holons.len() as i64));
                    let actual_count = holons.len().to_string();
                    println!("Success! DB has {actual_count} holons, as expected");

                } else {
                    panic!("Expected get_all_holons to return Holons response, but it didn't!");
                }
            }

        }
        Err(error)=> {
            panic!("{:?} Unable to build a get_all_holons request ", error);
        }
    }



}
