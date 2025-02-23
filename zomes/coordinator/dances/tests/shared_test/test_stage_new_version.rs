use crate::shared_test::test_data_types::{
    DanceTestState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
};
use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseBody::StagedRef, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_commit_dance_request, build_stage_new_version_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::reference_layer::{HolonReference, SmartReference};

use holons_core::core_shared_objects::{HolonCollection, RelationshipName};
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyName};
use std::collections::BTreeMap;

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    _conductor: &SweetConductor,
    _cell: &SweetCell,
    _test_state: &mut DanceTestState,
    _original_holon_key: MapString,
    _expected_response: ResponseStatusCode,
) -> () {
    info!("\n\n--- TEST STEP: Stage_New_Version ---- IS NOT CURRENTLY IMPLEMENTED :");

    // let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
    //
    // let original_holon = _test_state
    //     .created_holons
    //     .get(&_original_holon_key)
    //     .expect("Holon with key: {key} not found in created_holons");
    //
    // let original_holon_id = HolonId::Local(original_holon.get_local_id().unwrap());
    // // Build a stage_new_version DanceRequest
    // let request =
    //     build_stage_new_version_dance_request(&_test_state.session_state, original_holon_id.clone());
    // debug!("Dance Request: {:#?}", request);
    //
    // match request {
    //     Ok(valid_request) => {
    //         let response: DanceResponse =
    //             _conductor.call(&_cell.zome("dances"), "dance", valid_request).await;
    //         info!("Dance Response: {:#?}", response.clone());
    //         _test_state.session_state = response.state.clone();
    //         let code = response.status_code;
    //         assert_eq!(code.clone(), _expected_response);
    //         let description = response.description.clone();
    //
    //         if let ResponseStatusCode::OK = code {
    //             if let StagedRef(index) = response.body {
    //                 let index_value = index.to_string();
    //                 debug!("{index_value} returned in body");
    //                 // An index was returned in the body, retrieve the Holon at that index within
    //                 // the StagingArea and confirm it matches the expected Holon.
    //
    //                 let holons = response.state.get_staging_area().get_staged_holons();
    //
    //                 // debug!("holons:{:#?}", holons);
    //                 assert_eq!(
    //                     original_holon.essential_content(),
    //                     holons[index].essential_content(),
    //                 );
    //
    //                 let original_relationship_map = original_holon
    //                     .staged_relationship_map
    //                     .0
    //                     .clone()
    //                     .into_iter()
    //                     .filter(|(name, _)| *name != predecessor_relationship_name)
    //                     .collect::<BTreeMap<RelationshipName, HolonCollection>>();
    //
    //                 for (name, original_collection) in original_relationship_map {
    //                     let expected_collection = holons[index]
    //                         .relationship_map
    //                         .get_collection_for_relationship(&name)
    //                         .expect(&format!(
    //                             "{:?} relationship should exist in the returned holon",
    //                             name
    //                         ));
    //                     assert_eq!(
    //                         original_collection.get_keyed_index(),
    //                         expected_collection.get_keyed_index(),
    //                     );
    //                 }
    //                 let predecessor = &holons[index]
    //                     .relationship_map
    //                     .get_collection_for_relationship(&predecessor_relationship_name)
    //                     .expect(&format!(
    //                         "{:?} relationship should exist in the returned holon",
    //                         predecessor_relationship_name
    //                     ))
    //                     .get_members()[0];
    //                 assert_eq!(
    //                     predecessor,
    //                     &HolonReference::Smart(SmartReference::new(original_holon_id, None))
    //                 );
    //
    //                 info!("Success! DB fetched holon matched expected");
    //                 info!("Session State Returned is: {:?}", _test_state.session_state);
    //             } else {
    //                 panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
    //             }
    //         } else {
    //             panic!("DanceRequest returned {code} for {description}");
    //         }
    //     }
    //     Err(error) => {
    //         panic!("{:?} Unable to build a stage_new_from_clone request ", error);
    //     }
    // }
}
