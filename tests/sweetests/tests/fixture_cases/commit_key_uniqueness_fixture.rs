//! Committed historical keys remain reserved to their lineage after rekeying.
use super::described_instances::add_described_instance;
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationSubject, TestCaseInit,
};

pub fn commit_historical_key_uniqueness_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Historical key uniqueness",
        "A successor's rekey preserves its predecessor's key association",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let root = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.KeyHistory.A",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let update = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        root,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let update = test_case.add_with_properties_step(
        &mut fixture_holons,
        update,
        [("Key".to_property_name(), "Book.KeyHistory.B".to_base_value())].into(),
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(update, ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;
    test_case.add_begin_transaction_step(None, None)?;
    let collision = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.KeyHistory.A",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: collision,
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::RuleViolation { code: "DS-KEY-006".into() },
                rule_key: Some("KeyUniqueness.ValidationRule".into()),
                subject: ExpectedValidationSubject::Holon,
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
