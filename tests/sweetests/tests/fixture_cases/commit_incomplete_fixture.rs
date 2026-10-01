//! Declarative expectations for supported relationship-persistence failures and retries.

// Compiled into several test targets; each uses a subset of the fixture metadata.
#![allow(dead_code)]

use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, EdgeExpectation, ExpectedCommitCandidate, ExpectedCommitStatus,
    ExpectedDisposition, ExpectedPersistedEdge, ExpectedPersistedGraph, ExpectedRetryParticipant,
    PersistedSubject, TestCaseInit, TestReference,
};
use integrity_core_types::HolonErrorKind;

/// Finalized plan and identity tokens for injecting a conflict between staging and Commit.
pub struct IncompleteScenario {
    pub case: DancesTestCase,
    pub book: TestReference,
    pub title: TestReference,
    pub name: TestReference,
    pub saved_update: TestReference,
    pub first_attempt: usize,
}

/// Authors all three attempts before observing any response or injecting the conflict.
pub fn commit_incomplete_fixture() -> Result<IncompleteScenario, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit relationship failure and repeated retry",
        "Incomplete twice appends an error each time; repairing the conflict completes the same transaction",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let book = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.IncompleteRetry",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let book = fixture_holons.resolve_target_token_to_head(&book)?;
    test_case.add_begin_transaction_step(None, None)?;
    let update = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let mut targets = Vec::new();
    // This depends on collection member order surviving staging and Commit:
    // Name must persist before the fail-fast collection writer reaches the Title conflict.
    // Every retry must replay the earlier Name occurrence idempotently.
    for key in ["Name.PropertyType", "Title.PropertyType"] {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
        targets.push(test_case.add_lookup_saved_holon_by_key_step(
            &mut fixture_holons,
            stub,
            key,
            None,
            None,
        )?);
    }
    let name = targets[0].clone();
    let title = targets[1].clone();
    let update = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        update,
        "ReferencesProperty".to_relationship_name(),
        targets,
        None,
        None,
    )?;
    let companion = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.IncompleteCompanion",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let first_attempt = test_case.steps.len();
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        vec![
            ExpectedCommitCandidate::new(update.clone(), ExpectedDisposition::GraphOnly)
                .with_expected_new_errors(vec![HolonErrorKind::CommitFailure]),
            ExpectedCommitCandidate::new(companion.clone(), ExpectedDisposition::NewRoot),
        ],
        vec![],
        None,
        Some("Save nodes, then fail relationship persistence".into()),
    )?;
    let saved_update = fixture_holons.resolve_target_token_to_head(&update)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        vec![],
        vec![ExpectedRetryParticipant::new(saved_update.clone())
            .with_expected_new_errors(vec![HolonErrorKind::CommitFailure])],
        None,
        Some("Retry relationships with no live candidates; append the same error again".into()),
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![],
        vec![ExpectedRetryParticipant::new(saved_update.clone())],
        None,
        Some("Repair the conflict and finish the same transaction".into()),
    )?;
    test_case.add_match_saved_content_step()?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![
                PersistedSubject::Token(saved_update.clone()),
                PersistedSubject::Token(companion),
            ],
            edges: ["Name.PropertyType", "Title.PropertyType"]
                .into_iter()
                .map(|key| ExpectedPersistedEdge {
                    source: PersistedSubject::Token(saved_update.clone()),
                    relationship: "ReferencesProperty".to_relationship_name(),
                    inverse: Some("ReferencedByBook".to_relationship_name()),
                    target: PersistedSubject::Key(key.into()),
                    expectation: EdgeExpectation::ExactlyOnce,
                })
                .collect(),
            ..Default::default()
        },
        Some("Fresh reads verify replayed forward/inverse links without duplicates".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(IncompleteScenario { case: test_case, book, title, name, saved_update, first_attempt })
}
