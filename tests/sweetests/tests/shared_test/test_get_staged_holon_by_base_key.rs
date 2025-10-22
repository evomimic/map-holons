use holons_core::MockConductorConfig;
use holons_prelude::prelude::*;

use tracing::info;

use super::{
    // mock_conductor::MockConductorConfig,
    test_data_types::DanceTestExecutionState,
};

/// This function invokes the Nursery accessing its staged holons, to query by base key.
/// This executor is testing the convenience method for get_staged_holon_by_base_key
/// (Note: Holon 'singular' -- calling a function formerly known as get_staged_holon_by_key)
/// which has been transformed to base_key, as the current architecture needs to allow for the
/// possibility of staging multiple versioned Holons - that is, they are cloned from the same SmartReference
/// and could contain identical information during the staging process, therefore requiriing unique identifiers,
/// which will take the form of their "base_key" + "version_sequence_count".
///
/// IMPORTANT:
/// The test step calling this execution assumes that there is only one Holon with the associated base_key.
pub async fn execute_get_staged_holon_by_base_key(
    test_state: &mut DanceTestExecutionState<MockConductorConfig>,
    key: MapString,
) {
    info!("--- TEST STEP: Get Staged Holon By Base Key ---");

    // 1. Get context from test_state
    let context = test_state.context();

    // 2. Get Nursery access
    let nursery = context.get_space_manager().get_staging_service();
    // call the singular API to get the one staged holon
    let _staged_reference = nursery.read().unwrap().get_staged_holon_by_base_key(&key).unwrap();
}
