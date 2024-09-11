use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseBody::Index, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_stage_new_from_clone_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::holon::{self, Holon};
use holons::holon_reference::HolonReference;
use rstest::*;

use super::data_types::DanceTestState;

/// This function builds and dances a `stage_new_from_clone` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_from_clone(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    holon_reference: HolonReference,
    expected_response: ResponseStatusCode,
    expected_holon: Holon,
) -> () {
    info!("\n\n--- TEST STEP: Stage_New_From_Clone ---- :");

    // Build a stage_new_from_clone DanceRequest
    let request =
        build_stage_new_from_clone_dance_request(test_state.staging_area.clone(), holon_reference);
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

                    let holons = response.staging_area.staged_holons;

                    warn!("holons:{:#?}", holons);
                    assert_eq!(
                        expected_holon.essential_content(),
                        holons[index].essential_content(),
                    );
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
