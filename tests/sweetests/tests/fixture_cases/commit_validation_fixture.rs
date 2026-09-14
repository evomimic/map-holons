use super::described_instances::add_described_instance;
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{BOOK_DESCRIPTOR_KEY, PERSON_DESCRIPTOR_KEY};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationSubject, TestCaseInit,
};

/// A required property rule inherited from the PropertyType family rejects a
/// described Book, then accepts a corrected retry in the same open transaction.
pub fn commit_validation_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit validation correction",
        "Required Title rejection retains findings; correction commits nodes and relationships",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let book = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.ValidationCorrection",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let person = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Person.ValidationCorrection",
        "Name",
        PERSON_DESCRIPTOR_KEY,
    )?;
    let book = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book,
        "AuthoredBy".to_relationship_name(),
        vec![person],
        None,
        None,
    )?;
    let title: PropertyMap =
        [("Title".to_property_name(), "Corrected title".to_base_value())].into();
    let book = test_case.add_remove_properties_step(
        &mut fixture_holons,
        book,
        title.clone(),
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: book.clone(),
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::RuleViolation { code: "DS-PROP-001".into() },
                rule_key: Some("RequiredPropertyPresence.ValidationRule".into()),
                subject: ExpectedValidationSubject::Property("Title".into()),
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.add_with_properties_step(&mut fixture_holons, book, title, None, None)?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
