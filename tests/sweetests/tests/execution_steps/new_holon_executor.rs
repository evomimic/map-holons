use holons_prelude::prelude::*;
use holons_test::{ExecutionReference, ExecutionHandle, TestExecutionState, TestReference};
use integrity_core_types::PropertyMap;
use pretty_assertions::assert_eq;
use tracing::{debug, info};

/// This function creates a new holon with an optional key. It builds and dances a `new_holon` DanceRequest.
pub async fn execute_new_holon(
    state: &mut TestExecutionState,
    step_token: TestReference,
    properties: PropertyMap,
    key: Option<MapString>,
    expected_status: ResponseStatusCode,
    description:Option<String>,
) {
    let description = match description {
        Some(dsc) => dsc,
        None => "Creating a new Holon via DANCE".to_string()
    };
    info!("--- TEST STEP: {description} ---");

    let context = state.context();

    // 1. BUILD - the stage_new_holon DanceRequest
    let request = build_new_holon_dance_request(key);
    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request)
.await;
    info!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - response status
    assert_eq!(
        response.status_code, expected_status,
        "new_holon request failed: {}",
        response.description
    );

    // 4. RECORD — Register an ExecutionHolon so that this token becomes resolvable during test execution.
    let mut response_holon_reference = match response.body {
        ResponseBody::HolonReference(ref hr) => hr.clone(),
        other => {
            panic!("expected ResponseBody::HolonReference, got {:?}", other);
        }
    };

    // Apply property mutations returned by the dance
    for (name, value) in properties {
        response_holon_reference.with_property_value(name.clone(), value).unwrap_or_else(|error| {
            panic!("failed to set property {:?} on response holon: {}", name, error)
        });
    }

    // Build execution handle from runtime result
    let execution_handle = ExecutionHandle::from(response_holon_reference);

    // Canonical construction: token + execution outcome
    let execution_reference =
        ExecutionReference::from_token_execution(&step_token, execution_handle);

    // Validate expected vs execution-time content
    execution_reference.assert_essential_content_eq()
;
    info!("Success! Holon's essential content matched expected");

    // Record for downstream resolution
    state.record(&step_token, execution_reference).unwrap();
}
