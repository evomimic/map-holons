use std::{collections::VecDeque, path::PathBuf};

use holons_prelude::prelude::*;

use crate::shared_test::test_data_types::{
    map_core_schema_paths, DanceTestStep, DancesTestCase, TestSessionState,
};

/// Minimal loader-client fixture that feeds a single import JSON file into the
/// host-side loader client entrypoint and asserts a successful load.
///
/// The JSON fixture contains:
/// - One HolonLoaderBundle (implicit via filename)
/// - One HolonType descriptor holon
/// - One instance holon described by the type
pub async fn loader_client_fixture() -> Result<DancesTestCase, HolonError> {
    let mut steps = VecDeque::new();

    steps.push_back(DanceTestStep::LoadHolonsClient {
        import_files: map_core_schema_paths(),
        expect_staged: MapInteger(171),
        expect_committed: MapInteger(171),
        expect_links_created: MapInteger(741),
        expect_errors: MapInteger(0),
        expect_total_bundles: MapInteger(7),
        expect_total_loader_holons: MapInteger(171),
    });

    Ok(DancesTestCase {
        name: "loader_client_minimal".to_string(),
        description: "Minimal JSON loader input via loader_client entrypoint".to_string(),
        steps,
        test_session_state: TestSessionState::default(),
    })
}
