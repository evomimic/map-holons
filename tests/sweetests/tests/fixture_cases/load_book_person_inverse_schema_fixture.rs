use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Fixture for the `LoadBookPersonInverseTestSchema` preset step.
///
/// Loads MAP core schema first, then starts a fresh transaction and imports the
/// Book/Person inverse test schema through public MAP Commands `LoadHolons`
/// ingress. This exercises loader resolution against already-saved core-schema
/// holons rather than restaging core and domain together in one import.
pub fn load_book_person_inverse_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, .. } = TestCaseInit::new(
        "load_book_person_inverse_schema",
        "Load Book/Person inverse test schema after committed MAP core schema",
    );

    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
