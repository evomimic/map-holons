use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

pub fn load_core_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, .. } = TestCaseInit::new(
        "load_core_schema",
        "Load MAP core schema via LoadCoreSchema step",
    );

    test_case.add_load_core_schema_step(None)?;

    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
