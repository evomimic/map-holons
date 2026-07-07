use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

/// Loads the schemas needed for Book/Person instance resolution, then verifies
/// that public loader ingress rejects authoring an instance relationship through
/// the inverse `Authors` descriptor.
pub fn load_inverse_oriented_book_person_instances_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "load_inverse_oriented_book_person_instances",
        "Load core and Book/Person schemas, then reject an inverse-oriented Authors import",
    );

    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for Book/Person schema load".to_string()),
    )?;
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
