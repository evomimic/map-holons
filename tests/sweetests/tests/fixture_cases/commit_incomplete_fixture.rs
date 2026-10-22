//! Declarative expectations for supported relationship-persistence failures and retries.

// Compiled into several test targets; each uses a subset of the fixture metadata.
#![allow(dead_code)]

use super::described_instance_helpers::add_described_instance;
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

/// A retained graph-only replay rejects a conflicting live edit, then composes an addition.
pub struct GraphOverlapRetryScenario {
    pub case: DancesTestCase,
    pub source: TestReference,
    pub added: TestReference,
    pub first_attempt: usize,
    pub after_rejection: usize,
}

pub fn graph_overlap_retry_fixture() -> Result<GraphOverlapRetryScenario, HolonError> {
    use core_types::CommitValidationViolationKind;
    use holons_core::core_shared_objects::holon::ValidationState;
    use holons_test::{
        ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationSubject,
    };
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Graph-only overlapping retry",
        "Outstanding replay participates before writes and composes compatible live additions",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut source = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.GraphOverlapRetry",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let mut targets = Vec::new();
    for key in ["Name.PropertyType", "Title.PropertyType", "Key.PropertyType"] {
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
    source = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        source,
        "ReferencesProperty".to_relationship_name(),
        vec![targets[0].clone()],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let source = fixture_holons.resolve_target_token_to_head(&source)?;
    test_case.add_begin_transaction_step(None, None)?;
    let a = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        source.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let a = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        a,
        "ReferencesProperty".to_relationship_name(),
        vec![targets[1].clone()],
        None,
        None,
    )?;
    let first_attempt = test_case.steps.len();
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        vec![ExpectedCommitCandidate::new(a.clone(), ExpectedDisposition::GraphOnly)
            .with_expected_new_errors(vec![HolonErrorKind::CommitFailure])],
        vec![],
        None,
        None,
    )?;
    let a = fixture_holons.resolve_target_token_to_head(&a)?;
    // An Incomplete attempt retains the source transaction and its committed replay.
    let b = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        source.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let b = test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        b,
        "ReferencesProperty".to_relationship_name(),
        vec![targets[0].clone()],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: b.clone(),
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::RuleViolation {
                    code: "UnsupportedGraphOnlyCombination".into(),
                },
                rule_key: None,
                subject: ExpectedValidationSubject::Holon,
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            relationships: vec![ExpectedPersistedRelationship {
                source: PersistedSubject::Token(source.clone()),
                relationship: "ReferencesProperty".to_relationship_name(),
                targets: vec![PersistedSubject::Token(targets[0].clone())],
            }],
            ..Default::default()
        },
        Some("Rejection must prevent outstanding replay writes too".into()),
    )?;
    let after_rejection = test_case.steps.len();
    let c = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        source.clone(),
        None,
        MapInteger(3),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let c = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        c,
        "ReferencesProperty".to_relationship_name(),
        vec![targets[2].clone()],
        None,
        None,
    )?;
    test_case.add_abandon_staged_changes_step(&mut fixture_holons, b, None, None)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(c, ExpectedDisposition::GraphOnly)],
        vec![ExpectedRetryParticipant::new(a)],
        None,
        None,
    )?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            relationships: vec![ExpectedPersistedRelationship {
                source: PersistedSubject::Token(source.clone()),
                relationship: "ReferencesProperty".to_relationship_name(),
                targets: targets.iter().cloned().map(PersistedSubject::Token).collect(),
            }],
            ..Default::default()
        },
        Some("Fresh observer sees replayed and newly added targets".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(GraphOverlapRetryScenario {
        case: test_case,
        source,
        added: targets[1].clone(),
        first_attempt,
        after_rejection,
    })
}

/// A rekey publishes its node but fails its historical keyed Owns entry.
pub struct KeyIndexRetryScenario {
    pub case: DancesTestCase,
    pub root: TestReference,
    pub first_attempt: usize,
    pub after_rejection: usize,
    pub key: &'static str,
}

pub fn key_index_retry_fixture() -> Result<KeyIndexRetryScenario, HolonError> {
    use core_types::CommitValidationViolationKind;
    use holons_core::core_shared_objects::holon::ValidationState;
    use holons_test::{
        ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationSubject,
    };
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Historical key-index retry", "A published rekey reserves its key before index repair; retry restores the keyed Owns entry",
    );
    let key = "Book.KeyIndexRetry.New";
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let root = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.KeyIndexRetry.Old",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let root = fixture_holons.resolve_target_token_to_head(&root)?;
    test_case.add_begin_transaction_step(None, None)?;
    let update = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        root.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let update = test_case.add_with_properties_step(
        &mut fixture_holons,
        update,
        [("Key".to_property_name(), key.to_base_value())].into(),
        None,
        None,
    )?;
    let first_attempt = test_case.steps.len();
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        vec![ExpectedCommitCandidate::new(update.clone(), ExpectedDisposition::NewVersion)
            .with_expected_new_errors(vec![HolonErrorKind::CommitFailure])],
        vec![],
        None,
        None,
    )?;
    let published = fixture_holons.resolve_target_token_to_head(&update)?;
    let collision = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        key,
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: collision.clone(),
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
    let after_rejection = test_case.steps.len();
    test_case.add_abandon_staged_changes_step(&mut fixture_holons, collision, None, None)?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![],
        vec![ExpectedRetryParticipant::new(published)],
        None,
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(KeyIndexRetryScenario { case: test_case, root, first_attempt, after_rejection, key })
}
