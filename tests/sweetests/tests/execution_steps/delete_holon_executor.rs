use holons_prelude::prelude::*;
use holons_test::{TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

use holochain::sweettest::*;

use holons_core::core_shared_objects::ReadableHolonState;

/// This function builds and dances a `delete_holon` DanceRequest for the supplied Holon
/// and matches the expected response
///
pub async fn execute_delete_holon(
    context: &dyn HolonsContextBehavior,
    state: &mut TestExecutionState,
    source_token: TestReference,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Deleting an Existing (Saved) Holon");

    // 1) LOOKUP — get the input handle for the source token
    let source_reference: HolonReference =
        state.lookup_holon_reference(context, &source_token).unwrap();

    // 2. BUILD - dance request to commit
    let request = build_delete_holon_dance_request(local_id.clone())
        .expect("Failed to build delete_holon request");
    debug!("Dance Request: {:#?}", request);

    // 3. CALL - the dance
    let dance_initiator = context.get_space_manager().get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    debug!("Dance Response: {:#?}", response.clone());

    // 4. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "delete_holon request returned unexpected status: {}",
        response.description
    );
    info!("Success! DanceResponse matched expected");

    //
    // if expected_status == ResponseStatusCode::OK {
    //     info!("Success! delete_holon returned OK response, confirming deletion...");

    //     // 6. Verify the holon no longer exists
    //     let get_request = build_get_holon_by_id_dance_request(HolonId::Local(local_id.clone()))
    //         .expect("Failed to build get_holon_by_id request");

    //     let get_response = state.invoke_dance(get_request).await;
    //     assert_eq!(
    //         get_response.status_code,
    //         ResponseStatusCode::NotFound,
    //         "Holon should be deleted but was found"
    //     );

    //     info!("Confirmed Holon deletion!");
    // } else {
    //     info!("delete_holon matched expected response: {:?}", response.status_code);
    // }

    // 6) RECORD — tie the new staged handle to the **source token’s TemporaryId**
    //             so later steps can look it up with the same token.
    state.record_resolved(resolved_reference);
}
