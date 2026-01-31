use holons_prelude::prelude::*;
use holons_test::{ExecutionReference, ResultingReference, TestExecutionState, TestReference};
use integrity_core_types::PropertyMap;
use pretty_assertions::assert_eq;
use serde::de::value;
use tracing::{debug, info};

/// This function creates a new holon with an optional key. It builds and dances a `new_holon` DanceRequest.
pub async fn execute_new_holon(
    state: &mut TestExecutionState,
    source_token: TestReference,
    properties: PropertyMap,
    key: Option<MapString>,
    expected_status: ResponseStatusCode,
) {
    info!("--- TEST STEP: Creating a new Holon via DANCE ---");

    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // 1. BUILD - the stage_new_holon DanceRequest
    let request = build_new_holon_dance_request(key);
    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(context, request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "new_holon request failed: {}",
        response.description
    );

    // 4. RECORD - Register an ExecutionHolon so that this token becomes resolvable during test execution.
    let mut response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("{}", format!("expected ResponseBody::HolonReference, got {:?}", other));
        }
    };
    for (name, value) in properties {
        response_holon_reference.with_property_value(context, name, value);
    }

    let resulting_reference = ResultingReference::from(response_holon_reference);
    let resolved_reference = ExecutionReference::from_reference_parts(
        source_token.expected_snapshot(),
        resulting_reference,
    );
    resolved_reference.assert_essential_content_eq(context).unwrap();
    info!("Success! Holon's essential content matched expected");

    state.record(source_token.expected_id(), resolved_reference).unwrap();
}
