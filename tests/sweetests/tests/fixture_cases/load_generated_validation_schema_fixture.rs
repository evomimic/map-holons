use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Acceptance fixture proving that validation-owned bindings resolve against
/// already committed Core descriptors without changing Core's package boundary.
pub fn load_generated_validation_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "load_generated_validation_schema",
        "Load Core, then the generated validation package in a separate transaction",
    );
    test_case.add_load_generated_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_generated_validation_schema_step(None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
