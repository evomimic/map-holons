//! Commit preserves distinct sibling versions and graph edits anchored to their saved source.

use super::described_instance_helpers::{add_described_instance, saved};
use super::schema_authoring_helpers::{
    descriptor_properties, replace_target, stage_definition, stage_title,
};
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationSubject, TestCaseInit,
    TestReference,
};
use integrity_core_types::HolonErrorKind;

/// Distinct edits of one saved version publish distinct successors in one Commit.
/// Sharing a source, lineage and key must not collapse either candidate or saved result.
pub fn commit_version_siblings_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit version-producing siblings",
        "Two candidates publish independent versions with the same exact predecessor",
    );
    test_case.verify_committed_workset = true;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.VersionSiblings",
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
        Some("Stage first successor from A".into()),
    )?;
    // A singular base-key lookup is ambiguous, while both candidates remain staged.
    // Stage both before editing so the staging executor can compare their cloned content.
    let second = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        Some("Stage second successor from the same A".into()),
    )?;
    let first = test_case.add_with_properties_step(
        &mut fixture_holons,
        first,
        [("Title".to_property_name(), "Sibling B title".to_base_value())].into(),
        None,
        None,
    )?;
    let second = test_case.add_with_properties_step(
        &mut fixture_holons,
        second,
        [("Title".to_property_name(), "Sibling C title".to_base_value())].into(),
        None,
        None,
    )?;
    // Both candidates publish their own content. Their shared key belongs to one
    // lineage, so maintaining its saved key associations must permit both writes.
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(first.clone(), ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(second.clone(), ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    // The shared key still resolves through A's lineage, whose two heads make a
    // singular saved lookup ambiguous rather than selecting either sibling.
    test_case.add_begin_transaction_step(None, None)?;
    let key = MapString("Book.VersionSiblings".into());
    let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
    test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        stub,
        key,
        Some(HolonErrorKind::MultipleLineageHeads),
        Some("Verify the shared key reports both sibling heads".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

fn rejected_title(title: &TestReference) -> ExpectedRejectedHolon {
    ExpectedRejectedHolon {
        token: title.clone(),
        validation_state: ValidationState::Invalid,
        findings: vec![ExpectedValidationFinding {
            kind: CommitValidationViolationKind::RuleViolation { code: "DS-PROP-003".into() },
            rule_key: Some("NoUndescribedProperties.ValidationRule".into()),
            subject: ExpectedValidationSubject::Holon,
        }],
    }
}

/// Exact references stay exact; an invalid successor rejects independently without
/// blocking its explicit consumer.
pub fn commit_book_title_exact_references_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Book v1 and Title v2 exact references",
        "Invalid successors reject independently; explicit references publish only after correction",
    );
    test_case.verify_committed_workset = true;
    let case = &mut test_case;
    let holons = &mut fixture_holons;
    case.add_load_book_person_inverse_test_schema_step(None)?;
    case.add_begin_transaction_step(None, None)?;
    let owner = saved(&fixture_context, case, holons, "BookAuthorInverseSchema")?;
    let title_v1 =
        stage_title(&fixture_context, case, holons, "ExactReferences.Title.PropertyType", &owner)?;
    case.add_commit_step(holons, ExpectedCommitStatus::Complete, None, None)?;
    case.add_begin_transaction_step(None, None)?;
    // Book's authored target is pinned independently of Title's advancing fixture head.
    let pinned_title_v1 =
        saved(&fixture_context, case, holons, "ExactReferences.Title.PropertyType")?;
    let root = saved(&fixture_context, case, holons, "HolonType.TypeDescriptor")?;
    let meta = saved(&fixture_context, case, holons, "MetaHolonType.MetaTypeDescriptor")?;
    let mut properties = descriptor_properties("Book");
    properties.insert("InstanceDeletionAllowed".to_property_name(), true.to_base_value());
    let book_v1 = stage_definition(
        &fixture_context,
        case,
        holons,
        "ExactReferences.Book.HolonType",
        properties,
        &[
            ("Extends", &root),
            ("ComponentOf", &owner),
            ("InstanceProperties", &pinned_title_v1),
            ("DescribedBy", &meta),
        ],
    )?;
    case.add_commit_step(holons, ExpectedCommitStatus::Complete, None, None)?;
    case.add_begin_transaction_step(None, None)?;
    let pinned_book_v1 = saved(&fixture_context, case, holons, "ExactReferences.Book.HolonType")?;
    let title_v2 =
        case.add_stage_new_version_step(holons, title_v1, None, MapInteger(1), None, None)?;
    let title_v2 = case.add_with_properties_step(
        holons,
        title_v2,
        [
            ("UndescribedFixtureProperty".to_property_name(), true.to_base_value()),
            ("Description".to_property_name(), "Title successor".to_base_value()),
        ]
        .into(),
        None,
        None,
    )?;
    let unchanged_book = case.add_stage_new_version_step(
        holons,
        pinned_book_v1.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    case.add_commit_step(holons, ExpectedCommitStatus::Rejected, None, None)?;
    case.add_verify_commit_rejection_step(
        vec![rejected_title(&title_v2)],
        MapInteger(1),
        Some("Only Title v2 rejects; saved Book v1 remains a valid independent candidate".into()),
    )?;

    let book_v2 = case.add_stage_new_version_step(
        holons,
        book_v1.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let book_v2 =
        replace_target(case, holons, book_v2, "InstanceProperties", &pinned_title_v1, &title_v2)?;
    case.add_commit_step(holons, ExpectedCommitStatus::Rejected, None, None)?;
    case.add_verify_commit_rejection_step(
        vec![rejected_title(&title_v2)],
        MapInteger(1),
        Some("Only Title v2 rejects; its explicit Book v2 consumer remains valid".into()),
    )?;
    let title_v2 = case.add_remove_properties_step(
        holons,
        title_v2,
        [("UndescribedFixtureProperty".to_property_name(), true.to_base_value())].into(),
        None,
        None,
    )?;
    case.add_commit_step_with_dispositions(
        holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(title_v2.clone(), ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(unchanged_book, ExpectedDisposition::NoAction),
            ExpectedCommitCandidate::new(book_v2.clone(), ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// An unchanged candidate and a graph-only candidate share a saved definition,
/// while Commit handles their outcomes separately and writes only the graph edit.
pub fn commit_no_op_and_graph_only_siblings_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit no-op and graph-only siblings",
        "Both candidate results name the saved source and its graph edit creates no version",
    );
    test_case.verify_committed_workset = true;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.GraphOnlySiblings",
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
    let unchanged = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let graph_only = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        first,
        "ReferencesProperty".to_relationship_name(),
        vec![property.clone()],
        None,
        None,
    )?;
    // NoAction contributes no SavedHolons member and binds its token to A directly,
    // so A's single saved result belongs to the graph-only candidate.
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(unchanged.clone(), ExpectedDisposition::NoAction),
            ExpectedCommitCandidate::new(graph_only.clone(), ExpectedDisposition::GraphOnly),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Publishing branches in separate transactions preserves the exact source of each
/// successor, just as publishing siblings together does.
pub fn commit_branch_across_transactions_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit branches across transactions",
        "A to B and A to C both commit when staged in separate transactions",
    );
    test_case.verify_committed_workset = true;
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
    let branch_b = test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_b,
        branch_b_title,
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(branch_b.clone(), ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;

    test_case.add_match_saved_content_step()?;

    test_case.add_begin_transaction_step(None, None)?;
    let branch_c = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage branch C from A in a later transaction".into()),
    )?;
    let branch_c_title: PropertyMap =
        [("Title".to_property_name(), "Branch C title".to_base_value())].into();
    let branch_c = test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_c,
        branch_c_title,
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(branch_c.clone(), ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
