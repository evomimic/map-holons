//! Commit declarations exercise persistence outcomes independently of mutation policy.

// Compiled into several test targets; each uses a subset of these fixtures.
#![allow(dead_code)]

use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    FixtureHolons, TestCaseInit, TestReference,
};
use integrity_core_types::HolonErrorKind;

fn stage_update(
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    source: TestReference,
) -> Result<TestReference, HolonError> {
    test_case.add_stage_new_version_step(holons, source, None, MapInteger(1), None, None)
}

fn change_title(
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    candidate: TestReference,
    title: &str,
) -> Result<TestReference, HolonError> {
    test_case.add_with_properties_step(
        holons,
        candidate,
        [("Title".to_property_name(), title.to_base_value())].into(),
        None,
        None,
    )
}

/// An unchanged update saves nothing, but its token remains a saved source for later steps.
pub fn commit_no_action_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit unchanged update",
        "Zero saved results still bind the unchanged candidate to its saved source",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let source = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.NoAction",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let unchanged = stage_update(&mut test_case, &mut fixture_holons, source)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(unchanged.clone(), ExpectedDisposition::NoAction)],
        vec![],
        None,
        Some("Commit unchanged update with zero SavedHolons".into()),
    )?;
    test_case.add_match_saved_content_step()?;
    test_case.add_begin_transaction_step(None, None)?;
    let next = stage_update(&mut test_case, &mut fixture_holons, unchanged)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(next, ExpectedDisposition::NoAction)],
        vec![],
        None,
        Some("Reuse the unchanged candidate's saved-source identity".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// One attempt contains all four dispositions, with declarations in author order.
pub fn commit_mixed_dispositions_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit mixed dispositions",
        "Create, unchanged, graph-only, and new-version candidates share one Commit",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut sources = Vec::new();
    for key in ["Book.MixedUnchanged", "Book.MixedGraph", "Book.MixedVersion"] {
        sources.push(add_described_instance(
            &fixture_context,
            &mut test_case,
            &mut fixture_holons,
            key,
            "Title",
            BOOK_DESCRIPTOR_KEY,
        )?);
    }
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let unchanged = stage_update(&mut test_case, &mut fixture_holons, sources[0].clone())?;
    let graph = stage_update(&mut test_case, &mut fixture_holons, sources[1].clone())?;
    let property_key = MapString("Title.PropertyType".into());
    let stub = fixture_context.mutation().new_holon(Some(property_key.clone()))?;
    let property = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        stub,
        property_key,
        None,
        None,
    )?;
    let graph = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        graph,
        "ReferencesProperty".to_relationship_name(),
        vec![property],
        None,
        None,
    )?;
    let version = stage_update(&mut test_case, &mut fixture_holons, sources[2].clone())?;
    let version = change_title(&mut test_case, &mut fixture_holons, version, "Mixed new version")?;
    let root = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.MixedRoot",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(root, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(unchanged, ExpectedDisposition::NoAction),
            ExpectedCommitCandidate::new(graph, ExpectedDisposition::GraphOnly),
            ExpectedCommitCandidate::new(version, ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Two saved versions share a key but have distinct source and result identities.
pub fn commit_same_key_dispositions_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit distinguishable same-key results",
        "Versions of one key match by identity when declared in reverse staging order",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let key = "Book.SameKeyDispositions";
    let a = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        key,
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let b = stage_update(&mut test_case, &mut fixture_holons, a.clone())?;
    let b = change_title(&mut test_case, &mut fixture_holons, b, "Version B")?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(b.clone(), ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;
    test_case.add_begin_transaction_step(None, None)?;
    let from_b = stage_update(&mut test_case, &mut fixture_holons, b)?;
    // The staging executor's duplicate-key lookup checks equal property maps.
    // Prepare that condition before staging the second source, then differentiate them.
    let from_b = change_title(&mut test_case, &mut fixture_holons, from_b, key)?;
    let from_a = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        a,
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let from_a = change_title(&mut test_case, &mut fixture_holons, from_a, "Branch from A")?;
    let from_b = change_title(&mut test_case, &mut fixture_holons, from_b, "Successor of B")?;
    let root = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.SameKeyCompanion",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(from_a.clone(), ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(root, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(from_b.clone(), ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    // Each probe's execute_stage_new_version calls assert_expected_content_eq:
    // binding a token to the wrong same-key node would fail on its distinct Title.
    // Use separate transactions so each staging lookup checks one candidate at a time.
    test_case.add_begin_transaction_step(None, None)?;
    let from_b_probe = stage_update(&mut test_case, &mut fixture_holons, from_b)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(from_b_probe, ExpectedDisposition::NoAction)],
        vec![],
        None,
        None,
    )?;
    test_case.add_begin_transaction_step(None, None)?;
    let from_a_probe = stage_update(&mut test_case, &mut fixture_holons, from_a)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(from_a_probe, ExpectedDisposition::NoAction)],
        vec![],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// A property write promotes the candidate, while the declaration intentionally stays stale.
/// GraphOnly and NewVersion both return one saved result, so a count check cannot
/// distinguish them. The disposition check names the promotion before any downstream
/// snapshot failure, such as unexpected predecessor lineage.
pub fn commit_stale_graph_only_declaration_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit stale graph-only declaration",
        "Disposition diagnostics identify a promotion that saved-result counts cannot distinguish",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let source = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.StaleGraphOnly",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let update = stage_update(&mut test_case, &mut fixture_holons, source)?;
    let property_key = MapString("Title.PropertyType".into());
    let stub = fixture_context.mutation().new_holon(Some(property_key.clone()))?;
    let property = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        stub,
        property_key,
        None,
        None,
    )?;
    let update = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        update,
        "ReferencesProperty".to_relationship_name(),
        vec![property],
        None,
        None,
    )?;
    let update = change_title(&mut test_case, &mut fixture_holons, update, "Promoted version")?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(update, ExpectedDisposition::GraphOnly)],
        vec![],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// A semantically valid oversized string fails Pass 1 persistence preflight.
pub fn commit_unsupported_pass_one_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit unsupported Pass 1 failure",
        "An uncommitted create is unsupported and cannot be classified as no action",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let candidate = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.UnsupportedPassOne",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let candidate =
        change_title(&mut test_case, &mut fixture_holons, candidate, &"x".repeat(16_385))?;
    // PvlViolation records intent here: the unsupported-disposition panic occurs
    // before error_delta_report, so this fixture does not verify the error delta.
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        vec![ExpectedCommitCandidate::new(candidate, ExpectedDisposition::NewRoot)
            .with_expected_new_errors(vec![HolonErrorKind::PvlViolation])],
        vec![],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
