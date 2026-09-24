use super::described_instances::add_described_instance;
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationSubject, TestCaseInit,
};
use integrity_core_types::HolonErrorKind;

/// Two live updates of one saved source reject before persistence. Abandoning
/// one contender lets the other commit in the same transaction.
pub fn commit_competition_retry_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit competing replacements and retry",
        "Competing updates each retain a finding; reconciliation accepts a retry",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.CompetingCommit",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;

    let first = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage first replacement".into()),
    )?;
    let second = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original,
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        Some("Stage identical second replacement".into()),
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    let competition = ExpectedValidationFinding {
        kind: CommitValidationViolationKind::RuleViolation {
            code: "CompetingStagedReplacements".into(),
        },
        rule_key: None,
        subject: ExpectedValidationSubject::Holon,
    };
    test_case.add_verify_commit_rejection_step(
        vec![
            ExpectedRejectedHolon {
                token: first.clone(),
                validation_state: ValidationState::Invalid,
                findings: vec![competition.clone()],
            },
            ExpectedRejectedHolon {
                token: second.clone(),
                validation_state: ValidationState::Invalid,
                findings: vec![competition],
            },
        ],
        MapInteger(2),
        None,
    )?;
    test_case.add_abandon_staged_changes_step(&mut fixture_holons, second, None, None)?;
    // Keep the rejected competitors identical. The surviving update becomes a new version
    // only after reconciliation; an unchanged ForUpdate commits with NoAction.
    let retry_title: PropertyMap =
        [("Title".to_property_name(), "Reconciled title".to_base_value())].into();
    test_case.add_with_properties_step(
        &mut fixture_holons,
        first,
        retry_title,
        None,
        Some("Change the surviving replacement before retry".into()),
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// An unchanged update and a graph-only update still compete for one saved
/// source; neither persistence classification weakens the identity rule.
pub fn commit_graph_only_competition_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit graph-only competing replacement",
        "An unchanged update and a relationship-only update both receive competition findings",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.GraphOnlyCompetition",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let property_key = MapString("Title.PropertyType".into());
    let property_stub = fixture_context.mutation().new_holon(Some(property_key.clone()))?;
    let property = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        property_stub,
        property_key,
        None,
        None,
    )?;
    let first = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let second = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original,
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let graph_only = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        first,
        "ReferencesProperty".to_relationship_name(),
        vec![property],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    let competition = ExpectedValidationFinding {
        kind: CommitValidationViolationKind::RuleViolation {
            code: "CompetingStagedReplacements".into(),
        },
        rule_key: None,
        subject: ExpectedValidationSubject::Holon,
    };
    test_case.add_verify_commit_rejection_step(
        vec![
            ExpectedRejectedHolon {
                token: graph_only,
                validation_state: ValidationState::Invalid,
                findings: vec![competition.clone()],
            },
            ExpectedRejectedHolon {
                token: second,
                validation_state: ValidationState::Invalid,
                findings: vec![competition],
            },
        ],
        MapInteger(2),
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// A branch committed in a later transaction is distinct from two live
/// replacements competing in one Commit attempt.
pub fn commit_branch_across_transactions_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit branches across transactions",
        "A to B and A to C both commit when staged in separate transactions",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.BranchSource",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_begin_transaction_step(None, None)?;
    let branch_b = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage branch B from A".into()),
    )?;
    let branch_b_title: PropertyMap =
        [("Title".to_property_name(), "Branch B title".to_base_value())].into();
    test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_b,
        branch_b_title,
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_begin_transaction_step(None, None)?;
    let branch_c = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original,
        None,
        MapInteger(1),
        None,
        Some("Stage branch C from A in a later transaction".into()),
    )?;
    let branch_c_title: PropertyMap =
        [("Title".to_property_name(), "Branch C title".to_base_value())].into();
    test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_c,
        branch_c_title,
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
