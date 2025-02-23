use async_std::task;
use dances_core::dance_response::ResponseBody::Holons;
use dances_core::dance_response::{DanceResponse, ResponseStatusCode};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons_core::dances::holon_dance_adapter::{
    build_add_related_holons_dance_request, build_get_all_holons_dance_request,
    build_remove_related_holons_dance_request, build_with_properties_dance_request,
};

use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase};
use crate::shared_test::*;
use dances_core::dance_request::RequestBody::StagedRef;
use holons::reference_layer::staged_reference::StagedIndex;
use holons::reference_layer::{HolonReference, StagedReference};

use holons_core::core_shared_objects::{Holon, RelationshipName};
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapInteger, MapString};
use std::collections::BTreeMap;

/// This function is intended to test the ability to remove holons from a specified relationship
/// originating at a source_holon.
///
/// There are two levels of testing required.
/// 1. Removing related holons from an already staged holon.
/// 2. Removing related holons from a previously saved holon
///
/// The first is a local operation on the staged holon (it does not invoke any dances).
///
/// The second requires:
///     a. retrieving the source holon
///     b. either cloning it or staging a new version of it
///     c. removing the specified holons from the specified relationship
///     d. committing the changes
///     e. confirming the new holon is no longer related to the holons to remove via the specified relationship.
///

pub async fn execute_remove_related_holons(
    _conductor: &SweetConductor,
    _cell: &SweetCell,
    _test_state: &mut DanceTestExecutionState,
    _source_holon: StagedReference,
    _relationship_name: RelationshipName,
    _holons_to_remove: Vec<HolonReference>,
    _expected_response: ResponseStatusCode,
) -> () {
    warn!("\n\n--- TEST STEP: removing Related Holon is NOT CURRENTLY IMPLEMENTED");

    // // // Ensure source holon exists prior to dancing the request
    // // let _source_holon = test_state
    // //     .session_state
    // //     .get_staging_area()
    // //     .get_holon(source_holon_index)
    // //     .expect("Failed to get source holon from StagingArea");
    // //
    //
    // // Create the expected_holon from the source_holon + the supplied related holons
    //
    // // Build the DanceRequest
    // let request = build_remove_related_holons_dance_request(
    //     &test_state.session_state,
    //     source_holon,
    //     relationship_name,
    //     holons_to_remove,
    // );
    // info!("Dance Request: {:#?}", request);
    //
    // match request {
    //     Ok(valid_request) => {
    //         let response: DanceResponse =
    //             conductor.call(&cell.zome("dances"), "dance", valid_request).await;
    //         info!("Dance Response: {:#?}", response.clone());
    //         let code = response.status_code;
    //
    //         test_state.session_state = response.state.clone();
    //
    //         assert_eq!(code, expected_response);
    //         info!("as expected, remove_related_holons dance request returned {:#?}", code.clone());
    //
    //         if let ResponseStatusCode::OK = code {
    //             if let StagedRef(index) = response.body {
    //                 let index_value = index.to_string();
    //                 info!("{index_value} returned in body");
    //                 // An index was returned in the body, retrieve the Holon at that index within
    //                 // the StagingArea and confirm it matches the expected Holon.
    //
    //                 let source_holon_in_response = response
    //                     .state
    //                     .get_staging_area()
    //                     .get_holon(index)
    //                     .expect("Failed to get source holon in DanceResponse");
    //                 assert_eq!(source_holon_in_response, expected_holon);
    //
    //                 info!("Success! Related Holons have been removed");
    //             } else {
    //             }
    //         }
    //     }
    //     Err(error) => {
    //         panic!("{:?} Unable to build a stage_holon request ", error);
    //     }
    // }
}
