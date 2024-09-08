//! Holon Descriptor Test Cases

#![allow(unused_imports)]

use async_std::task;
use dances::dance_response::ResponseBody::{Holons, Index};
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_add_related_holons_dance_request, build_get_all_holons_dance_request, build_remove_related_holons_dance_request, build_with_properties_dance_request
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::commit_manager::StagedIndex;
use holons::holon_reference::HolonReference;
use pretty_assertions::assert_eq;
use rstest::*;
use std::collections::BTreeMap;

use crate::shared_test::test_data_types::DanceTestStep;
use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};

/// This function builds and dances a `add_related_holons` DanceRequest for the supplied relationship
/// and holons
///

pub async fn execute_remove_related_holons(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    source_holon_index: StagedIndex,
    relationship_name: RelationshipName,
    holons_to_remove: Vec<HolonReference>,
    expected_response: ResponseStatusCode,
    expected_holon: Holon,
) -> () {
    info!(
        "\n\n--- TEST STEP: removing Related Holons. Expecting  {:#?}",
        expected_response.clone()
    );

    // Get the state of the holon prior to dancing the request
    let source_holon = test_state
        .staging_area
        .staged_holons
        .get(source_holon_index);
    if source_holon.is_none() {
        panic!(
            "Unable to get source holon from the staging_area at index  {:#?}",
            source_holon_index.to_string()
        );
    }

    // Create the expected_holon from the source_holon + the supplied related holons


    // Build the DanceRequest
    let request = build_remove_related_holons_dance_request(
        test_state.staging_area.clone(),
        source_holon_index,
        relationship_name,
        holons_to_remove,
    );
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
            info!(
                "as expected, remove_related_holons dance request returned {:#?}",
                code.clone()
            );

            if let ResponseStatusCode::OK = code {
                if let Index(index) = response.body {
                    let index_value = index.to_string();
                    info!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let source_holon = response.staging_area.staged_holons[index].clone();

                    assert_eq!(expected_holon, source_holon);

                    info!("Success! Related Holons have been removed");
                } else {
                }
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a stage_holon request ", error);
        }
    }
}
