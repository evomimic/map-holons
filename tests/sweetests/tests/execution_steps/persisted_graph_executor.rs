//! Fresh persisted graph checks independent of saved-content equivalence policy.

use super::persisted_read_support::{
    find_holons_by_key, fresh_related_holon_ids, fresh_related_holon_members,
    loaded_holons_with_fresh_context, local_id,
};
use holons_prelude::prelude::*;
use holons_test::{
    assert_edge_occurrences, assert_exact_relationship_ids, EdgeExpectation,
    ExpectedPersistedGraph, PersistedSubject, ResolveBy, TestExecutionState,
};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing::info;

/// Checks enumerated subjects, forward/inverse occurrences, and exact target sets for
/// declared relationships and lineage.
/// One fresh observer transaction leaves the active Commit/retry transaction untouched.
pub async fn execute_verify_persisted_graph(
    state: &mut TestExecutionState,
    expected: ExpectedPersistedGraph,
) {
    let (context, enumerated) =
        loaded_holons_with_fresh_context(state, "verify_persisted_graph").await;
    let enumerated_ids: Vec<_> = enumerated.get_members().iter().map(local_id).collect();
    let resolve = |subject| resolve_subject(state, &context, &enumerated, subject);
    for subject in &expected.enumerated {
        let reference = resolve(subject);
        assert_edge_occurrences(
            &format!("get-all subject {subject:?}"),
            &enumerated_ids,
            &local_id(&reference),
            EdgeExpectation::ExactlyOnce,
        );
    }
    for edge in &expected.edges {
        let source = resolve(&edge.source);
        let target = resolve(&edge.target);
        let source_id = local_id(&source);
        let target_id = local_id(&target);
        let relationship = edge.relationship.to_string();
        assert_edge_occurrences(
            &format!("source {source_id:?} --{relationship}--> target {target_id:?}"),
            &fresh_related_holon_ids(&source, &relationship),
            &target_id,
            edge.expectation,
        );
        if let Some(inverse) = &edge.inverse {
            let inverse = inverse.to_string();
            assert_edge_occurrences(
                &format!("inverse target {target_id:?} --{inverse}--> source {source_id:?}"),
                &fresh_related_holon_ids(&target, &inverse),
                &source_id,
                edge.expectation,
            );
        }
    }
    // Lineage expands into ordinary exact-target assertions so one comparator governs
    // every exactness claim, whether the relationship is fixed or author-declared.
    // Each lineage subject resolves once and serves both directions, because a
    // traversal subject would otherwise repeat its hop reads per direction.
    let mut exact = Vec::new();
    for relationship in &expected.relationships {
        exact.push((
            resolve(&relationship.source),
            relationship.relationship.to_string(),
            &relationship.targets,
        ));
    }
    for lineage in &expected.lineage {
        let subject = resolve(&lineage.subject);
        exact.push((subject.clone(), "Predecessor".to_string(), &lineage.predecessors));
        exact.push((subject, "Successor".to_string(), &lineage.successors));
    }
    for (source, relationship, targets) in exact {
        let target_ids: Vec<_> = targets.iter().map(|target| local_id(&resolve(target))).collect();
        assert_exact_relationship_ids(
            &format!("source {:?} --{relationship}--> exact targets", local_id(&source)),
            fresh_related_holon_ids(&source, &relationship),
            target_ids,
        );
    }
    info!(
        "verified persisted graph: {} enumerated subjects, {} edges, {} exact relationships, {} lineage subjects",
        expected.enumerated.len(),
        expected.edges.len(),
        expected.relationships.len(),
        expected.lineage.len()
    );
}

/// Resolves saved identity only; snapshot content and staged relationships are never compared.
fn resolve_subject(
    state: &TestExecutionState,
    context: &Arc<TransactionContext>,
    enumerated: &HolonCollection,
    subject: &PersistedSubject,
) -> HolonReference {
    match subject {
        PersistedSubject::Token(token) => {
            let reference = state
                .resolve_execution_reference(context, ResolveBy::Expected, token)
                .unwrap_or_else(|error| panic!("persisted subject {subject:?}: {error:?}"));
            assert!(
                matches!(reference, HolonReference::Smart(_)),
                "persisted subject {subject:?} must resolve to a saved reference"
            );
            // Discard projected property hints; this assertion uses the saved identity only.
            HolonReference::smart_from_id(
                context.space_read_handle(),
                reference.holon_id().unwrap(),
            )
        }
        PersistedSubject::Key(key) => {
            let matches = find_holons_by_key(enumerated, key);
            assert_eq!(
                matches.len(),
                1,
                "persisted subject key {key}: expected exactly one enumerated identity, found {}",
                matches.len()
            );
            matches[0].clone()
        }
        PersistedSubject::Successor { of, generation } => {
            let mut reference = resolve_subject(state, context, enumerated, of);
            for hop in 1..=*generation {
                let successors = fresh_related_holon_members(&reference, "Successor");
                assert_eq!(successors.len(), 1,
                    "persisted subject {subject:?}, hop {hop}: expected exactly one Successor, got {:?}",
                    successors.iter().map(local_id).collect::<Vec<_>>());
                reference = successors[0].clone();
            }
            reference
        }
    }
}
