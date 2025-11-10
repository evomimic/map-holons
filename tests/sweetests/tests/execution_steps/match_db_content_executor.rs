use holons_test::TestExecutionState;
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

pub async fn execute_match_db_content(context: &dyn HolonsContextBehavior, test_state: &mut TestExecutionState, expected_status: ResponseStatusCode) {
    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    let ctx_arc = test_state.context(); // Arc lives until end of scope
    let context = ctx_arc.as_ref();

    // 1. ITERATE - through all created holons and verify them in the database
    for (_key, expected_holon) in test_state.holons().by_temporary_id {
        // Get HolonId
        let holon_id: HolonId =
            expected_holon.holon_id(context).expect("Failed to get HolonId").into();

        // 2) BUILD — get_holon_by_id DanceRequest
        let request = build_get_holon_by_id_dance_request(holon_id.clone())
            .expect("Failed to build get_holon_by_id request");
        debug!("Dance Request: {:#?}", request);

        // 3) CALL — the dance
        let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
        let response = dance_initiator.initiate_dance(context, request).await;

        // 4. VALIDATE - Ensure response contains the expected Holon
        
        if let ResponseBody::Holon(actual_holon) = response.body {
            assert_eq!(
                expected_holon.key(context),
                actual_holon.key(),
                "Holon content mismatch for ID {:?}",
                holon_id
            );

            resolved_reference.assert_essential_content_eq(context).unwrap();
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
