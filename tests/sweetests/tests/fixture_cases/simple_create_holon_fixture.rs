use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationSubject, TestCaseInit,
};
use rstest::*;
use std::collections::BTreeMap;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _fixture_bindings,
    } = TestCaseInit::new(
        "Simple Create/Get Holon Testcase",
        "Undescribed creation is rejected with identity-only findings and an open transaction",
    );

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_key = MapString("Book.SimpleCreate".to_string());
    let book_transient_reference = fixture_context.mutation().new_holon(Some(book_key.clone()))?;

    let mut properties = BTreeMap::new();
    properties.insert("title".to_property_name(), book_key.clone().to_base_value());
    properties.insert("description".to_property_name(), "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_base_value());
    // Mint
    let book_step_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_transient_reference,
        properties,
        Some(book_key),
        None,
        Some("Creating book holon... ".to_string()),
    )?;

    let staged = test_case.add_stage_holon_step(
        &mut fixture_holons,
        book_step_token.clone(),
        None,
        Some("Staging book holon...".to_string()),
    )?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;

    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: staged,
            validation_state: ValidationState::NoDescriptor,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::NoDescriptor,
                rule_key: None,
                subject: ExpectedValidationSubject::Holon,
            }],
        }],
        MapInteger(1),
        None,
    )?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
