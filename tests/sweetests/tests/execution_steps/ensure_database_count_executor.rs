use holons_test::TestExecutionState;
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function builds and dances a `get_all_holons` DanceRequest and confirms that the number
/// of holons returned matches the expected_count of holons provided.
///

pub async fn execute_ensure_database_count(
    state: &mut TestExecutionState,
    expected_count: MapInteger,
) {
    info!("--- TEST STEP: Ensuring database holds {} holons ---", expected_count.0);

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. BUILD - the get_all_holons DanceRequest
    let request =
        build_get_all_holons_dance_request().expect("Failed to build get_all_holons request");

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response contains Holons
    if let ResponseBody::HolonCollection(holon_collection) = response.body {
        let actual_count = holon_collection.get_count();
        debug!(
            "--- TEST STEP ensure_db_count: Expected: {:?}, Retrieved: {:?} Holons ---",
            expected_count, actual_count.0
        );

        // 4. ASSERT - that the expected count matches actual count
        assert_eq!(expected_count, actual_count);
        info!("Success! DB count matched expected");
    } else {
        panic!(
            "Expected ensure_database_count to return {} holons, but it returned an unexpected response: {:?}",
            expected_count.0, response.body
        );
    }
}
