use std::collections::BTreeMap;

use async_std::task;
use dances::dance_response::ResponseBody;
use dances::dance_response::{DanceResponse, ResponseStatusCode};
use dances::holon_dance_adapter::{
    build_abandon_staged_changes_dance_request, build_get_all_holons_dance_request,
    build_get_holon_by_id_dance_request, build_stage_new_holon_dance_request,
};
use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons::commit_manager::StagedIndex;
use holons::context::HolonsContext;
use rstest::*;

use crate::shared_test::dance_fixtures::*;
use crate::shared_test::test_data_types::DanceTestStep;
use crate::shared_test::test_data_types::{DanceTestState, DancesTestCase};
use crate::shared_test::*;
use holons::helpers::*;
use holons::holon::Holon;
use holons::holon_api::*;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonGettable;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapBoolean, MapInteger, MapString};

/// This function builds and dances an `abandon_staged_changes` DanceRequest,
/// If the `ResponseStatusCode` returned by the dance != `expected`, panic to fail the test
/// Otherwise, if the dance returns an `OK` response,
///     confirm the Holon is in an `Abandoned` state and attempt various operations
///     that should be `NotAccessible` for holons an `Abandoned` state. If any of them do NOT
///     return a `NotAccessible` error, then panic to fail the test
/// Log a `info` level message marking the test step as Successful and return
///
pub async fn execute_abandon_staged_changes(
    conductor: &SweetConductor,
    cell: &SweetCell,
    test_state: &mut DanceTestState,
    staged_index: StagedIndex,
    expected_response: ResponseStatusCode,
) {
    info!("\n\n--- TEST STEP: Abandon Staged Changes ---");
    let request =
        build_abandon_staged_changes_dance_request(&test_state.session_state, staged_index.clone());

    info!("Dance Request: {:#?}", request);

    match request {
        Ok(valid_request) => {
            let response: DanceResponse =
                conductor.call(&cell.zome("dances"), "dance", valid_request).await;
            test_state.session_state = response.state.clone();

            assert_eq!(response.status_code, expected_response);

            info!("As expected, Dance returned {:?}", response.status_code);

            match response.status_code {
                ResponseStatusCode::OK => {
                    // Dance response was OK, confirm that operations disallowed for Holons in an
                    // Abandoned state return NotAccessible error.
                    if let ResponseBody::Index(staged_index) = response.body.clone() {
                        match test_state
                            .session_state
                            .get_staging_area_mut()
                            .get_holon_mut(staged_index)
                        {
                            Ok(abandoned_holon) => {
                                // NOTE: We changed the access policies to ALLOW read access to
                                // Abandoned holons, so disabling the following two checks
                                // TODO: Consider adding checks that these operations are ALLOWED
                                // assert!(matches!(
                                //     abandoned_holon.get_property_value(
                                //         &PropertyName(MapString("some_name".to_string()))
                                //     ),
                                //     Err(HolonError::NotAccessible(_, _))
                                // ));
                                // debug!("Confirmed abandoned holon is NotAccessible for `get_property_value`");
                                //
                                // assert!(matches!(
                                //     abandoned_holon.get_key(),
                                //     Err(HolonError::NotAccessible(_, _))
                                // ));
                                // debug!("Confirmed abandoned holon is NotAccessible for `get_key`");

                                assert!(matches!(
                                    abandoned_holon.with_property_value(
                                        PropertyName(MapString("some_name".to_string())),
                                        BaseValue::BooleanValue(MapBoolean(true))
                                    ),
                                    Err(HolonError::NotAccessible(_, _))
                                ));
                                debug!("Confirmed abandoned holon is NotAccessible for `with_property_value`");

                                // TODO: support get_all_related_holons required for this assertion
                                // assert!(matches!(
                                //     abandoned_holon.get_related_holons(None),
                                //     Err(HolonError::NotAccessible(_, _))
                                // ));
                                // debug!("Confirmed abandoned holon is NotAccessible for `get_related_holons`");
                            }
                            Err(e) => {
                                panic!("Failed to get holon: {:?}", e);
                            }
                        }
                    } else {
                        panic!("Expected abandon_staged_changes to return an Index response, but it didn't");
                    }
                }
                _ => (),
            }
        }
        Err(error) => {
            panic!("{:?} Unable to build a abandon_staged_changes request ", error);
        }
    }
}
