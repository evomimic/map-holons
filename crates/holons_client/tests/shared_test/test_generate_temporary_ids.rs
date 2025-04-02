use holon_dance_builders::generate_temporary_ids::build_generate_temporary_ids_dance_request;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use shared_types_holon::MapInteger;
use tracing::{debug, info};

use super::test_data_types::DanceTestExecutionState;
use crate::mock_conductor::MockConductorConfig;

/// This function
pub async fn execute_generate_temporary_ids(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    amount: MapInteger,
) {

    info!("--- TEST STEP: Ensuring database matches expected holons ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Build commit DanceRequest (state is handled inside dance_call)
    let request = build_generate_temporary_ids_dance_request(amount.clone())
        .expect("Failed to build commit DanceRequest");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(context, request);
    debug!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code,
        ResponseStatusCode::OK,
        "Commit request failed: {}",
        response.description
    );
    info!("Success! Commit succeeded");

    // 5. Extract saved Holons from response body and add them to `created_holons`
    match response.body {
        ResponseBody::TemporaryIds(ids) => {
            assert_eq!(ids.len() as i64, amount.0);
            info!("--- TEST STEP generate_temporary_ids: yielded: {:#?} \n", ids);
        }
        _ => panic!("Invalid ResponseBody: {:?}", response.body),
    }
}
