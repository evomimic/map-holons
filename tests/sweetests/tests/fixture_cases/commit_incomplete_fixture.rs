//! Declarative expectations for supported relationship-persistence failures and retries.

// Compiled into several test targets; each uses a subset of the fixture metadata.
#![allow(dead_code)]

use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedPersistedGraph, ExpectedPersistedRelationship, ExpectedRetryParticipant, FixtureHolons,
    PersistedSubject, TestCaseInit, TestReference,
};
use integrity_core_types::HolonErrorKind;

/// Finalized plan and identity tokens for injecting a conflict between staging and Commit.
pub struct IncompleteScenario {
    pub case: DancesTestCase,
    pub attempts: Vec<IncompleteAttempts>,
}

/// A three-attempt sequence, with or without an unchanged live candidate.
pub struct IncompleteAttempts {
    pub book: TestReference,
    pub title: TestReference,
    pub name: TestReference,
    pub saved_update: TestReference,
    pub first_attempt: usize,
    pub end_step: usize,
    pub unchanged: Option<UnchangedParticipant>,
}

/// The persisted source, retained staged candidate, and its binding after Complete.
pub struct UnchangedParticipant {
    pub source: TestReference,
    pub staged: TestReference,
    pub saved: TestReference,
}

/// Authors both retry worksets on one runtime: zero live candidates, then a retained NoAction.
pub fn commit_incomplete_fixture() -> Result<IncompleteScenario, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit relationship failure and repeated retry",
        "Repeated relationship errors support both zero-live-candidate retries and a retained unchanged candidate",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    let first = add_incomplete_attempts(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "Book.IncompleteRetry",
        false,
        &[],
    )?;
    let second = add_incomplete_attempts(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "Book.IncompleteWithNoAction",
        true,
        &[first.saved_update.clone()],
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(IncompleteScenario { case: test_case, attempts: vec![first, second] })
}

/// Earlier sequences remain known inverse sources because the runtime is shared within this case.
fn add_incomplete_attempts(
    test_case: &mut DancesTestCase,
    fixture_context: &std::sync::Arc<TransactionContext>,
    fixture_holons: &mut FixtureHolons,
    book_key: &str,
    include_unchanged: bool,
    prior_inverse_sources: &[TestReference],
) -> Result<IncompleteAttempts, HolonError> {
    test_case.add_begin_transaction_step(None, None)?;
    let book = add_described_instance(
        fixture_context,
        test_case,
        fixture_holons,
        book_key,
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let unchanged_source = if include_unchanged {
        Some(add_described_instance(
            fixture_context,
            test_case,
            fixture_holons,
            &format!("{book_key}.Unchanged"),
            "Title",
            BOOK_DESCRIPTOR_KEY,
        )?)
    } else {
        None
    };
    test_case.add_commit_step(fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let book = fixture_holons.resolve_target_token_to_head(&book)?;
    test_case.add_begin_transaction_step(None, None)?;
    let update = test_case.add_stage_new_version_step(
        fixture_holons,
        book.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let unchanged = unchanged_source
        .map(|source| {
            let source = fixture_holons.resolve_target_token_to_head(&source)?;
            let staged = test_case.add_stage_new_version_step(
                fixture_holons,
                source.clone(),
                None,
                MapInteger(1),
                None,
                None,
            )?;
            Ok::<_, HolonError>((source, staged))
        })
        .transpose()?;
    let mut targets = Vec::new();
    // This depends on collection member order surviving staging and Commit:
    // Name must persist before the fail-fast collection writer reaches the Title conflict.
    // Every retry must replay the earlier Name occurrence idempotently.
    for key in ["Name.PropertyType", "Title.PropertyType"] {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
        targets.push(test_case.add_lookup_saved_holon_by_key_step(
            fixture_holons,
            stub,
            key,
            None,
            None,
        )?);
    }
    let name = targets[0].clone();
    let title = targets[1].clone();
    let update = test_case.add_add_related_holons_step(
        fixture_holons,
        update,
        "ReferencesProperty".to_relationship_name(),
        targets,
        None,
        None,
    )?;
    let companion = add_described_instance(
        fixture_context,
        test_case,
        fixture_holons,
        &format!("{book_key}.Companion"),
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let first_attempt = test_case.steps.len();
    let unchanged_declaration = || {
        unchanged.as_ref().map(|(_, staged)| {
            ExpectedCommitCandidate::new(staged.clone(), ExpectedDisposition::NoAction)
        })
    };
    let mut candidates = vec![
        ExpectedCommitCandidate::new(update.clone(), ExpectedDisposition::GraphOnly)
            .with_expected_new_errors(vec![HolonErrorKind::CommitFailure]),
        ExpectedCommitCandidate::new(companion.clone(), ExpectedDisposition::NewRoot),
    ];
    candidates.extend(unchanged_declaration());
    test_case.add_commit_step_with_dispositions(
        fixture_holons,
        ExpectedCommitStatus::Incomplete,
        candidates,
        vec![],
        None,
        Some("Save nodes, then fail relationship persistence".into()),
    )?;
    let saved_update = fixture_holons.resolve_target_token_to_head(&update)?;
    test_case.add_commit_step_with_dispositions(
        fixture_holons,
        ExpectedCommitStatus::Incomplete,
        unchanged_declaration().into_iter().collect(),
        vec![ExpectedRetryParticipant::new(saved_update.clone())
            .with_expected_new_errors(vec![HolonErrorKind::CommitFailure])],
        None,
        Some("Retry the remaining workset; append the same relationship error again".into()),
    )?;
    test_case.add_commit_step_with_dispositions(
        fixture_holons,
        ExpectedCommitStatus::Complete,
        unchanged_declaration().into_iter().collect(),
        vec![ExpectedRetryParticipant::new(saved_update.clone())],
        None,
        Some("Repair the conflict and finish the same transaction".into()),
    )?;
    let unchanged = unchanged
        .map(|(source, staged)| {
            let saved = fixture_holons.resolve_target_token_to_head(&staged)?;
            Ok::<_, HolonError>(UnchangedParticipant { source, staged, saved })
        })
        .transpose()?;
    test_case.add_match_saved_content_step()?;
    // Both sequences own the isolated runtime, so all earlier inverse sources are known.
    // An extra or duplicated link in either direction fails here.
    let referenced = ["Name.PropertyType", "Title.PropertyType"];
    let mut relationships = vec![ExpectedPersistedRelationship {
        source: PersistedSubject::Token(saved_update.clone()),
        relationship: "ReferencesProperty".to_relationship_name(),
        targets: referenced.iter().map(|key| PersistedSubject::Key((*key).into())).collect(),
    }];
    relationships.extend(referenced.iter().map(|key| {
        ExpectedPersistedRelationship {
            source: PersistedSubject::Key((*key).into()),
            relationship: "ReferencedByBook".to_relationship_name(),
            targets: prior_inverse_sources
                .iter()
                .chain(std::iter::once(&saved_update))
                .cloned()
                .map(PersistedSubject::Token)
                .collect(),
        }
    }));
    test_case.add_verify_persisted_graph_step(
        fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![
                PersistedSubject::Token(saved_update.clone()),
                PersistedSubject::Token(companion),
            ],
            relationships,
            ..Default::default()
        },
        Some("Fresh reads verify exact replayed forward and inverse collections".into()),
    )?;
    Ok(IncompleteAttempts {
        book,
        title,
        name,
        saved_update,
        first_attempt,
        end_step: test_case.steps.len(),
        unchanged,
    })
}
