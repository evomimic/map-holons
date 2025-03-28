use crate::shared_test::test_data_types::{
    DanceTestExecutionState, DanceTestStep, DancesTestCase, TestHolonData, TestReference,
};

use holochain::sweettest::*;
use holochain::sweettest::{SweetCell, SweetConductor};

use crate::shared_test::mock_conductor::MockConductorConfig;
// use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::proptest::test_runner::contextualize_config;
use holon_dance_builders::stage_new_version_dance::build_stage_new_version_dance_request;
use holons_core::core_shared_objects::{HolonCollection, RelationshipName};
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::{HolonCollectionApi, HolonReadable, HolonReference, SmartReference};
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyName};
use std::collections::BTreeMap;
use tracing::{debug, info};

/// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
/// and confirms a Success response
///
pub async fn execute_stage_new_version(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    original_holon_key: MapString,
    expected_response: ResponseStatusCode,
) {
    info!("--- TEST STEP: Staging a New Version of a Holon ---");

    let predecessor_relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));

    // 1. Get context from test_state
    let context = test_state.context();

    // 1. Retrieve the original Holon
    let original_holon =
        test_state.get_created_holon_by_key(&original_holon_key).unwrap_or_else(|| {
            panic!("Holon with key {:?} not found in created_holons", original_holon_key)
        });

    let original_holon_id =
        HolonId::Local(original_holon.get_local_id().expect("Failed to get LocalId"));

    // 2. Build the DanceRequest
    let request = build_stage_new_version_dance_request(original_holon_id.clone())
        .expect("Failed to build stage_new_version request");

    debug!("Dance Request: {:#?}", request);

    // 3. Call the dance
    let response = test_state.dance_call_service.dance_call(test_state.context(), request).await;
    info!("Dance Response: {:#?}", response.clone());

    // 4. Validate response status
    assert_eq!(
        response.status_code, expected_response,
        "stage_new_version request returned unexpected status: {}",
        response.description
    );

    // 5. If successful, verify the new version
    if response.status_code == ResponseStatusCode::OK {
        if let ResponseBody::StagedRef(new_version_holon) = response.body {
            debug!("New version Holon reference returned: {:?}", new_version_holon);

            // Ensure essential content is preserved
            assert_eq!(
                original_holon.essential_content(),
                new_version_holon.essential_content(context),
                "New version Holon content did not match original"
            );

            /* TODO: re-consider how to validate relationships -- need ALL populated relationships

            // Validate relationships (excluding predecessor relationship)
            let original_relationship_map: BTreeMap<RelationshipName, HolonCollection> =
                original_holon
                    .staged_relationship_map
                    .0
                    .clone()
                    .into_iter()
                    .filter(|(name, _)| *name != predecessor_relationship_name)
                    .collect();

            for (name, original_collection) in original_relationship_map {
                let expected_collection = new_version_holon
                    .relationship_map
                    .get_collection_for_relationship(&name)
                    .expect(&format!("{:?} relationship should exist in the returned holon", name));
                assert_eq!(
                    original_collection.get_keyed_index(),
                    expected_collection.get_keyed_index(),
                    "Mismatch in relationship {:?}",
                    name
                );
            }
            */

            // Validate predecessor relationship
            // Get the related holons into a longer-lived variable
            let related_holons = new_version_holon
                .get_related_holons(context, &predecessor_relationship_name)
                .expect(&format!(
                    "{:?} relationship should exist in the returned holon",
                    predecessor_relationship_name
                ));

            // Extract the first member
            let predecessor = related_holons
                .get_by_index(0)
                .expect("Predecessor relationship should contain at least one member");

            assert_eq!(
                predecessor,
                HolonReference::Smart(SmartReference::new(original_holon_id, None)),
                "Predecessor relationship did not match expected"
            );

            info!("Success! New version Holon matched expected content and relationships.");
        } else {
            panic!("Expected StagedRef in response body, but got {:?}", response.body);
        }
    }
}
