use std::collections::BTreeMap;

use dances_core::dance_response::ResponseBody;
use dances_core::dance_response::{DanceResponse, ResponseBody::StagedRef, ResponseStatusCode};
use holons_core::dances::holon_dance_adapter::build_stage_new_from_clone_dance_request;
use hdk::prelude::*;
use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::sodoken::crypto_box::curve25519xchacha20poly1305::SEALBYTES;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use holons::reference_layer::{HolonReference, SmartReference};

use holons_core::core_shared_objects::RelationshipName;
use rstest::*;
use shared_types_holon::{HolonId, MapString};

use crate::shared_test::test_data_types::{
    DanceTestExecutionState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
};

/// This function builds and dances a `stage_new_from_clone` DanceRequest for the supplied
/// TestReference and confirms a Success response.
///
/// The implementation of this step consists of the following stages:
///      1. Construct the HolonReference to the original holon required by the DanceRequest
///      2. Build the DanceRequest
///      3. Dance the DanceRequest
///      4. Confirm the actual result matches the expect result
///
/// The `stage_new_from_clone_dance` is a Clone method that uses a `HolonReference` to identify the
/// Holon to clone. This means that to build the dance request, we need to create a `HolonReference`
/// from the `original_test_ref :  TestReference`.
///
/// The `original_test_ref` can either be a:
/// - `Saved` variant that holds the key for the previously saved holon to clone
/// - `Staged` variant that holds a StagedReference to the Nursery resident holon to clone.
///
/// To get the `HolonReference` for the `Saved` case, we need to:
///      1. retrieve the holon via its key from the test_state
///      2. get its `HolonId`
///      3. create a `SmartReference` for the `HolonId` and wrap the `SmartReference` in a `HolonReference`
///
///  To get the `HolonReference` in the `Staged case`, we simply need to wrap the `StagedReference`
///  in a `HolonReference`
pub async fn execute_stage_new_from_clone(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestExecutionState,
    original_test_ref: TestReference,
    expected_response: ResponseStatusCode,
) -> () {
    // TODO: Replace this literal with the CoreType Name for the PREDECESSOR relationship
    let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));

    // *** STAGE 1: Construct the HolonReference to the original holon required by the DanceRequest
    let original_holon_ref: HolonReference = match original_test_ref {
        TestReference::StagedHolon(staged_reference) => HolonReference::Staged(staged_reference),

        TestReference::SavedHolon(key) => {
            let saved_holon = test_state
                .get_created_holon_by_key(&key)
                .expect("Holon with key: {key} not found in created_holons");
            let local_id = saved_holon.get_local_id().unwrap();
            HolonReference::Smart(SmartReference::new(
                HolonId::Local(local_id),
                Some(saved_holon.property_map.clone()),
            ))
        }
    };
    let original_holon = original_holon_ref.clone_holon();

    // *** STAGE 2: Build a stage_new_from_clone DanceRequest
    let request = build_stage_new_from_clone_dance_request(
        &test_state.session_state,
        original_holon_data.holon_reference,
    );
    debug!("Dance Request: {:#?}", request);

    // *** STAGE 3: Dance the request

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                _conductor.call(&_cell.zome("dances"), "dance", valid_request).await;
            debug!("Dance Response: {:#?}", response.clone());

            // *** STAGE 4: Confirm the actual result matches the expect result

            test_state.session_state = response.state;
            let code = response.status_code;
            assert_eq!(code.clone(), _expected_response);
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
