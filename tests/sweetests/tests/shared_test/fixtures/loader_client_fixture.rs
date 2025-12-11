use std::{collections::VecDeque, fs, path::PathBuf};

use holons_loader_client::{ContentSet, FileData, BOOTSTRAP_IMPORT_SCHEMA_PATH};
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

    let schema_path = PathBuf::from(BOOTSTRAP_IMPORT_SCHEMA_PATH);
    let schema = FileData {
        filename: schema_path.display().to_string(),
        raw_contents: fs::read_to_string(&schema_path)
            .expect("failed to read bootstrap import schema for loader_client_fixture"),
    };

    let files_to_load: Vec<FileData> = map_core_schema_paths()
        .into_iter()
        .map(|path| FileData {
            filename: path.display().to_string(),
            raw_contents: fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("failed to read core schema file {}: {e}", path.display())
            }),
        })
        .collect();

    let content_set = ContentSet { schema, files_to_load };

    steps.push_back(DanceTestStep::LoadHolonsClient {
        content_set,
        expect_staged: MapInteger(182),
        expect_committed: MapInteger(182),
        expect_links_created: MapInteger(741),
        expect_errors: MapInteger(0),
        expect_total_bundles: MapInteger(7),
        expect_total_loader_holons: MapInteger(182),
    });

    Ok(DancesTestCase {
        name: "loader_client_minimal".to_string(),
        description: "Core Schema JSON loader input via loader_client entrypoint".to_string(),
        steps,
        test_session_state: TestSessionState::default(),
    })
}
