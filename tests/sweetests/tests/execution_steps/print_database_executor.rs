use holons_test::TestExecutionState;
use tracing::{debug, info};

use holons_prelude::prelude::*;

/// This function retrieves all holons and then writes log messages for each holon:
/// `info!` -- writes only the "key" for each holon
/// `debug!` -- writes the full json-formatted contents of the holon
///
pub async fn execute_print_database(state: &mut TestExecutionState) {
    info!("--- TEST STEP: Print Database Contents ---");

    let context = state.context();

    // 1. BUILD - the get_all_holons DanceRequest
    let request =
        build_get_all_holons_dance_request().expect("Failed to build get_all_holons request");

    debug!("Dance Request: {:#?}", request);

    // 2. CALL - the dance
    let dance_initiator = context.get_dance_initiator().unwrap();
    let response = dance_initiator.initiate_dance(&context, request)
.await;
    debug!("Dance Response: {:#?}", response.clone());

    // 3. VALIDATE - verify response contains Holons
    if let ResponseBody::HolonCollection(holons) = response.body {
        info!("DB contains {} holons", holons.get_count());

        for holon in holons {
            let key = holon
                .key()
                .map(|key| key.unwrap_or_else(|| MapString("<None>".to_string())))
                .unwrap_or_else(|err| {
                    panic!("Attempt to key() resulted in error: {:?}", err);
                });

            info!("Key = {:?}", key.0);
            info!("{:?}", holon.summarize());
            // debug!("Holon JSON: {:?}", as_json(&holon));
        }
    } else {
        panic!("Expected print_database to return Holons response, but got {:?}", response.body);
    }
}
