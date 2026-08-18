use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Proves that the Query package loads after Core and that its Dance adapter
/// loads only after the independently usable Query package.
pub fn load_generated_query_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "load_generated_query_schema",
        "Load Core, then the generated Query and Query Dance Adapter packages in separate transactions",
    );
    test_case.add_load_generated_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_generated_query_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_generated_query_dance_schema_step(None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
