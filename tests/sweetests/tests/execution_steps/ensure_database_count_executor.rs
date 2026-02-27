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
    description: String,
) {
    let context = state.context();

    // 1. BUILD - the get_all_holons DanceRequest
    let request =
        build_get_all_holons_dance_request().expect("Failed to build get_all_holons request");

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response contains Holons
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "get_all_holons returned unexpected status: {}",
        response.description
    );

    let holon_collection = match response.body {
        ResponseBody::HolonCollection(collection) => collection,
        other => panic!("Expected get_all_holons to return HolonCollection, got {:?}", other),
    };

    let actual_count = holon_collection.get_count();
    debug!(
        "--- TEST STEP ensure_db_count: Expected: {:?}, Retrieved: {:?} Holons ---",
        expected_count, actual_count.0
    );

    // 4. ASSERT - that the expected count matches actual count
    assert_eq!(expected_count, actual_count);
    info!("Success! DB count matched expected");
}
