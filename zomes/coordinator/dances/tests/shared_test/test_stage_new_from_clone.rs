use std::collections::BTreeMap;

use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseBody::StagedReference, ResponseStatusCode};
use dances::holon_dance_adapter::build_stage_new_from_clone_dance_request;
use hdk::prelude::*;
use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::sodoken::crypto_box::curve25519xchacha20poly1305::SEALBYTES;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use holons::reference_layer::{HolonReference, SmartReference};
use holons::shared_objects_layer::RelationshipName;
use rstest::*;
use shared_types_holon::{HolonId, MapString};

use crate::shared_test::test_data_types::{
    DanceTestState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
};

/// This function builds and dances a `stage_new_from_clone` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_from_clone(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    original_holon: TestReference,
    expected_response: ResponseStatusCode,
) -> () {
    info!("\n\n--- TEST STEP: Stage_New_From_Clone ---- :");

    let _predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));

    let original_holon_data: TestHolonData = match original_holon {
        TestReference::StagedHolon(staged_reference) => {
            let holon_reference = HolonReference::Staged(staged_reference);
            TestHolonData::new(staged_holon.clone(), holon_reference)
        }
        TestReference::SavedHolon(key) => {
            let saved_holon = test_state
                .created_holons
                .get(&key)
                .expect("Holon with key: {key} not found in created_holons");
            let local_id = saved_holon.get_local_id().unwrap();
            let holon_reference = HolonReference::Smart(SmartReference::new(
                HolonId::Local(local_id),
                Some(saved_holon.property_map.clone()),
            ));
            TestHolonData::new(saved_holon.clone(), holon_reference)
        }
    };
    let original_holon = original_holon_data.holon;
    // Build a stage_new_from_clone DanceRequest
    let request = build_stage_new_from_clone_dance_request(
        &test_state.session_state,
        original_holon_data.holon_reference,
    );
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            debug!("Dance Response: {:#?}", response.clone());
            test_state.session_state = response.state;
            let code = response.status_code;
            assert_eq!(code.clone(), expected_response);
            let description = response.description.clone();

            if let ResponseStatusCode::OK = code {
                if let StagedReference(index) = response.body {
                    let index_value = index.to_string();
                    debug!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let holons = test_state.session_state.get_staging_area().get_staged_holons();

                    // debug!("holons:{:#?}", holons);
                    assert_eq!(
                        original_holon.essential_content(),
                        holons[index].essential_content(),
                    );

                    // let original_relationship_map = original_holon
                    //     .relationship_map
                    //     .0
                    //     .into_iter()
                    //     .filter(|(name, _)| *name != predecessor_relationship_name)
                    //     .collect::<BTreeMap<RelationshipName, HolonCollection>>();

                    // for (name, original_collection) in original_relationship_map {
                    //     let expected_collection = holons[index]
                    //         .relationship_map
                    //         .get_collection_for_relationship(&name)
                    //         .expect(&format!(
                    //             "{:?} relationship should exist in the returned holon",
                    //             name
                    //         ));
                    //     assert_eq!(original_collection, *expected_collection);
                    // }

                    info!("Success! DB fetched holon matched expected");
                } else {
                    panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
                }
            } else {
                panic!("DanceRequest returned {code} for {description}");
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a stage_new_from_clone request ", error);
        }
    }
}
