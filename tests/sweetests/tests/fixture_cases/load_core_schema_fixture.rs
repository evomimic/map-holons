use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Pilot fixture for the `LoadCoreSchema` precondition step.
///
/// Exercises public MAP Commands `LoadHolons` ingress end-to-end by loading the MAP
/// core schema as a single preset step. The runtime then routes through
/// `holons_loader_client::load_holons_from_files`, so this covers the same
/// loader-client path that the earlier `loader_client_fixture` was built to exercise.
pub fn load_core_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, .. } =
        TestCaseInit::new("load_core_schema", "Load MAP core schema via LoadCoreSchema step");

    test_case.add_load_core_schema_step(None)?;

    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
