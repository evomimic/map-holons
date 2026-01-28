use holons_test::{TestExecutionState, TestHolonState};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

// use base_types::{MapInteger, MapString};
use core_types::HolonId; // TODO: Eliminate this dependency

use holons_client::init_client_context;
use holons_core::{core_shared_objects::ReadableHolonState, dances::ResponseBody}; // TODO: Eliminate this dependency
                                                                                  // use holons_guest_integrity::HolonNode;

/// This function iterates through the expected_holons vector supplied as a parameter
/// and for each holon: builds and dances a `get_holon_by_id` DanceRequest,
/// then confirms that the Holon returned matches the expected

pub async fn execute_match_db_content(state: &mut TestExecutionState) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // Iterate through all created holons and verify them in the database, panic if resolved reference does not match expected state
    for (id, resolved_reference) in state.holons().by_snapshot_id.clone() {
        if resolved_reference.expected_snapshot.state() == TestHolonState::Saved {
            let holon_reference = resolved_reference
                .resulting_reference
                .get_holon_reference()
                .expect("HolonReference must be Live, cannot be in a deleted state");
            if !matches!(holon_reference, HolonReference::Smart(_)) {
                panic!(
                    "Expected resulting_reference for id: {:?} to be Smart, but got {:?}",
                    id, resolved_reference.resulting_reference
                );
            }

            let holon_id = holon_reference.holon_id(context).expect("Failed to get HolonId");

            // 2. BUILD — get_holon_by_id DanceRequest
            let request = build_get_holon_by_id_dance_request(holon_id.clone())
                .expect("Failed to build get_holon_by_id request");
            debug!("Dance Request: {:#?}", request);

            // 3. CALL — the dance
            let dance_initiator = context.get_dance_initiator().unwrap();
            let response = dance_initiator.initiate_dance(context, request).await;

            // 4. VALIDATE - Ensure response contains the expected Holon
            if let ResponseBody::Holon(actual_holon) = response.body {
                assert_eq!(
                    resolved_reference.expected_snapshot.essential_content(context).unwrap(),
                    actual_holon.essential_content(),
                );
                info!(
                    "SUCCESS! DB fetched holon matched expected for: \n {:?}",
                    actual_holon.summarize()
                );
            } else {
                panic!(
                    "Expected get_holon_by_id to return a Holon response for id: {:?}, but got {:?}",
                    holon_id, response.body
                );
            }
        }
    }
}
