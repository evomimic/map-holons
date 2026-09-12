use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Requires inverse-oriented imports to fail loader resolution before Commit.
/// This regression intentionally remains red until loader-side direction
/// enforcement is restored; partial persistence is not the expected contract.
pub fn load_inverse_oriented_book_person_instances_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "load_inverse_oriented_book_person_instances",
        "Inverse-oriented AuthorOf import reports Skipped with a loader error and no committed holons",
    );

    // Core is provided by harness bootstrap. The suite loads this domain schema
    // at most once, so this scenario also works without reimporting saved Core.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_verify_book_person_descriptors_step(None)?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for inverse-oriented instance import".to_string()),
    )?;
    test_case.add_load_inverse_oriented_book_person_instances_expect_failure_step(None)?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
