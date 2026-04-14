use holons_prelude::prelude::*;
use holons_test::harness::helpers::{build_core_schema_content_set, CORE_SCHEMA_METRICS};
use holons_test::{DancesTestCase, TestCaseInit};

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

    let content_set = build_core_schema_content_set()?;

    test_case.add_load_holons_client_step(
        content_set,
        MapInteger(CORE_SCHEMA_METRICS.staged),
        MapInteger(CORE_SCHEMA_METRICS.committed),
        MapInteger(CORE_SCHEMA_METRICS.links_created),
        MapInteger(CORE_SCHEMA_METRICS.errors),
        MapInteger(CORE_SCHEMA_METRICS.total_bundles),
        MapInteger(CORE_SCHEMA_METRICS.total_loader_holons),
    )?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
