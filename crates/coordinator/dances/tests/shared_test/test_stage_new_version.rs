use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseBody::Index, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_commit_dance_request, build_stage_new_version_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::holon::{self, Holon};
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::smart_reference::SmartReference;
use holons::staged_reference::StagedReference;
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyName};

use super::data_types::DanceTestState;

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    original_holon_key: MapString,
    expected_response: ResponseStatusCode,
) -> () {
    info!("\n\n--- TEST STEP: Stage_New_Version ---- :");

    let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));

    let original_holon = test_state
        .created_holons
        .get(&original_holon_key)
        .expect("Holon with key: {key} not found in created_holons");
    let holon_id = HolonId::Local(original_holon.get_local_id().unwrap());
    // Build a stage_new_version DanceRequest
    let request = build_stage_new_version_dance_request(test_state.staging_area.clone(), holon_id);
    debug!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse = conductor
                .call(&cell.zome("dances"), "dance", valid_request)
                .await;
            debug!("Dance Response: {:#?}", response.clone());
            test_state.staging_area = response.staging_area.clone();
            let code = response.status_code;
            assert_eq!(code.clone(), expected_response);
            let description = response.description.clone();

            if let ResponseStatusCode::OK = code {
                if let Index(index) = response.body {
                    let index_value = index.to_string();
                    debug!("{index_value} returned in body");
                    // An index was returned in the body, retrieve the Holon at that index within
                    // the StagingArea and confirm it matches the expected Holon.

                    let holons = response.staging_area.get_staged_holons();

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
            panic!(
                "{:?} Unable to build a stage_new_from_clone request ",
                error
            );
        }
    }
}
