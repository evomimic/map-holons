use core_types::{ContentSet, FileData};
use holons_loader_client::BOOTSTRAP_IMPORT_SCHEMA_PATH;
use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};
use std::{fs, path::PathBuf};

/// Absolute paths to all core schema import files used for loader-client testing.
pub fn map_core_schema_paths() -> Vec<PathBuf> {
    // CARGO_MANIFEST_DIR for these tests points to `tests/sweetests`,
    // so we need to walk back to the repo root before joining the import_files path.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("host");

    let rels = [
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-abstract-value-types.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-concrete-value-types.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-dance-schema.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-keyrules-schema.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-property-types.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-relationship-types.json",
        "import_files/map-schema/core-schema/MAP Schema Types-map-core-schema-root.json",
    ];

    rels.iter().map(|rel| repo_root.join(rel)).collect()
}

/// Minimal loader-client fixture that feeds a single import JSON file into the
/// host-side loader client entrypoint and asserts a successful load.
///
/// The JSON fixture contains:
/// - One HolonLoaderBundle (implicit via filename)
/// - One HolonType descriptor holon
/// - One instance holon described by the type
pub fn loader_client_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, .. } = TestCaseInit::new(
        "loader_client_minimal",
        "Core Schema JSON loader input via loader_client entrypoint",
    );

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

    test_case.add_load_holons_client_step(
        content_set,
        MapInteger(182),
        MapInteger(182),
        MapInteger(1060),
        MapInteger(0),
        MapInteger(7),
        MapInteger(182),
    )?;
    // test_case.add_load_holons_client_step(
    // content_set,
    // expect_staged: MapInteger(182),
    // expect_committed: MapInteger(182),
    // expect_links_created: MapInteger(1060),
    // expect_errors: MapInteger(0),
    // expect_total_bundles: MapInteger(7),
    // expect_total_loader_holons: MapInteger(182),
    // });

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
