use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Acceptance fixture for the checked-in Schema 2.0 generated Core artifact.
pub fn load_generated_core_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "load_generated_core_schema",
        "Load generated Schema 2.0 Core JSON through the generic Holon Loader",
    );
    test_case.add_load_generated_core_schema_step(None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
