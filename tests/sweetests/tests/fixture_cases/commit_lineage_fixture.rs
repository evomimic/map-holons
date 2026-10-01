//! Saved content and exact persisted ancestry across successive Commit attempts.

use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, EdgeExpectation, ExpectedCommitCandidate, ExpectedCommitStatus,
    ExpectedDisposition, ExpectedLineage, ExpectedPersistedEdge, ExpectedPersistedGraph,
    FixtureHolons, PersistedSubject, TestCaseInit, TestReference,
};

fn stage_update(
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    source: TestReference,
) -> Result<TestReference, HolonError> {
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_stage_new_version_step(holons, source, None, MapInteger(1), None, None)
}

fn commit_candidate(
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    candidate: TestReference,
    disposition: ExpectedDisposition,
) -> Result<(), HolonError> {
    test_case.add_commit_step_with_dispositions(
        holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(candidate, disposition)],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()
}

fn publish_version(
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    source: TestReference,
    title: &str,
) -> Result<TestReference, HolonError> {
    let candidate = stage_update(test_case, holons, source)?;
    let candidate = test_case.add_with_properties_step(
        holons,
        candidate,
        [("Title".to_property_name(), title.to_base_value())].into(),
        None,
        None,
    )?;
    commit_candidate(test_case, holons, candidate.clone(), ExpectedDisposition::NewVersion)?;
    Ok(candidate)
}

fn lineage(
    subject: &TestReference,
    predecessors: &[&TestReference],
    successors: &[&TestReference],
) -> ExpectedLineage {
    ExpectedLineage {
        subject: PersistedSubject::Token(subject.clone()),
        predecessors: predecessors
            .iter()
            .map(|token| PersistedSubject::Token((*token).clone()))
            .collect(),
        successors: successors
            .iter()
            .map(|token| PersistedSubject::Token((*token).clone()))
            .collect(),
    }
}

/// Each successor has only its immediate predecessor; ancestry does not accumulate.
pub fn commit_sequential_lineage_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit sequential lineage",
        "Saved-content comparison and fresh reads verify A to B to C immediate ancestry",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let a = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.SequentialLineage",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let b = publish_version(&mut test_case, &mut fixture_holons, a.clone(), "Sequential B")?;
    let c = publish_version(&mut test_case, &mut fixture_holons, b.clone(), "Sequential C")?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![
                PersistedSubject::Token(a.clone()),
                PersistedSubject::Key("Book.SequentialLineage".into()),
            ],
            lineage: vec![
                lineage(&a, &[], &[&b]),
                lineage(&b, &[&a], &[&c]),
                lineage(&c, &[&b], &[]),
            ],
            ..Default::default()
        },
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Branches committed in separate transactions share only their declared source.
pub fn commit_branch_lineage_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit branch lineage",
        "Two same-key branches have exact predecessors and both appear in the root's successors",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let a = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.BranchLineage",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let b = publish_version(&mut test_case, &mut fixture_holons, a.clone(), "Branch B")?;
    let c = publish_version(&mut test_case, &mut fixture_holons, a.clone(), "Branch C")?;
    // Tokens identify each branch even though both saved versions have the same key.
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![
                PersistedSubject::Token(a.clone()),
                PersistedSubject::Key("Book.BranchLineage".into()),
            ],
            lineage: vec![
                lineage(&a, &[], &[&b, &c]),
                lineage(&b, &[&a], &[]),
                lineage(&c, &[&a], &[]),
            ],
            ..Default::default()
        },
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Unchanged and graph-only updates retain non-root ancestry; independent clones reset it.
pub fn commit_non_root_lineage_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit non-root lineage and independent clone",
        "Reuse B without losing B to A, replay graph links once, and clone B into an unrelated root",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let a = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.NonRootLineage",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    let b = publish_version(&mut test_case, &mut fixture_holons, a.clone(), "Non-root B")?;
    let unchanged = stage_update(&mut test_case, &mut fixture_holons, b.clone())?;
    commit_candidate(
        &mut test_case,
        &mut fixture_holons,
        unchanged.clone(),
        ExpectedDisposition::NoAction,
    )?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            lineage: vec![lineage(&a, &[], &[&unchanged]), lineage(&unchanged, &[&a], &[])],
            ..Default::default()
        },
        Some("Unchanged B retains its predecessor A and saved identity".into()),
    )?;

    let mut graph = unchanged;
    for property_key in ["Title.PropertyType", "Name.PropertyType"] {
        let candidate = stage_update(&mut test_case, &mut fixture_holons, graph.clone())?;
        let key = MapString(property_key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
        let property = test_case.add_lookup_saved_holon_by_key_step(
            &mut fixture_holons,
            stub,
            key,
            None,
            None,
        )?;
        graph = test_case.add_add_related_holons_step(
            &mut fixture_holons,
            candidate,
            "ReferencesProperty".to_relationship_name(),
            vec![property],
            None,
            None,
        )?;
        // The second graph-only attempt re-persists the collection containing Title,
        // exercising suppression of an already persisted forward/inverse pair.
        commit_candidate(
            &mut test_case,
            &mut fixture_holons,
            graph.clone(),
            ExpectedDisposition::GraphOnly,
        )?;
        let mut edges = Vec::new();
        for target in ["Title.PropertyType", "Name.PropertyType"] {
            edges.push(ExpectedPersistedEdge {
                source: PersistedSubject::Token(graph.clone()),
                relationship: "ReferencesProperty".to_relationship_name(),
                inverse: Some("ReferencedByBook".to_relationship_name()),
                target: PersistedSubject::Key(target.into()),
                expectation: if target == "Name.PropertyType"
                    && property_key == "Title.PropertyType"
                {
                    EdgeExpectation::Absent
                } else {
                    EdgeExpectation::ExactlyOnce
                },
            });
            edges.push(ExpectedPersistedEdge {
                source: PersistedSubject::Token(a.clone()),
                relationship: "ReferencesProperty".to_relationship_name(),
                inverse: Some("ReferencedByBook".to_relationship_name()),
                target: PersistedSubject::Key(target.into()),
                expectation: EdgeExpectation::Absent,
            });
        }
        test_case.add_verify_persisted_graph_step(
            &fixture_holons,
            ExpectedPersistedGraph {
                edges,
                lineage: vec![lineage(&a, &[], &[&graph]), lineage(&graph, &[&a], &[])],
                ..Default::default()
            },
            Some("Graph-only B retains ancestry and exact forward/inverse edge occurrences".into()),
        )?;
    }

    // Give B both predecessor and successor lineage before independently cloning it.
    let c = publish_version(&mut test_case, &mut fixture_holons, graph.clone(), "Non-root C")?;
    test_case.add_begin_transaction_step(None, None)?;
    let clone = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        graph.clone(),
        MapString("Book.IndependentNonRootClone".into()),
        None,
        None,
    )?;
    commit_candidate(
        &mut test_case,
        &mut fixture_holons,
        clone.clone(),
        ExpectedDisposition::NewRoot,
    )?;
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![
                PersistedSubject::Token(a.clone()),
                PersistedSubject::Token(clone.clone()),
                PersistedSubject::Key("Book.NonRootLineage".into()),
                PersistedSubject::Key("Book.IndependentNonRootClone".into()),
            ],
            lineage: vec![
                lineage(&a, &[], &[&b]),
                lineage(&graph, &[&a], &[&c]),
                lineage(&c, &[&graph], &[]),
                lineage(&clone, &[], &[]),
            ],
            ..Default::default()
        },
        Some(
            "Independent clone has neither predecessor nor successor; source lineage survives"
                .into(),
        ),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
