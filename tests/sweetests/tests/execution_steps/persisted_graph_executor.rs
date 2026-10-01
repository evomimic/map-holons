//! Fresh persisted graph checks independent of saved-content equivalence policy.

use super::persisted_read_support::{
    find_holons_by_key, fresh_related_holon_ids, fresh_related_holon_members,
    loaded_holons_with_fresh_context, local_id,
};
use holons_prelude::prelude::*;
use holons_test::{
    EdgeExpectation, ExpectedPersistedGraph, PersistedSubject, ResolveBy, TestExecutionState,
};
use integrity_core_types::LocalId;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing::info;

/// Checks enumerated subjects, forward/inverse occurrences, and exact lineage identities.
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
    for lineage in &expected.lineage {
        let subject = resolve(&lineage.subject);
        for (name, targets) in
            [("Predecessor", &lineage.predecessors), ("Successor", &lineage.successors)]
        {
            let target_ids: Vec<_> =
                targets.iter().map(|target| local_id(&resolve(target))).collect();
            assert_exact_relationship_ids(
                &format!("subject {:?} {name}", local_id(&subject)),
                fresh_related_holon_ids(&subject, name),
                target_ids,
            );
        }
    }
    info!(
        "verified persisted graph: {} enumerated subjects, {} edges, {} lineage subjects",
        expected.enumerated.len(),
        expected.edges.len(),
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

/// Counts occurrences rather than identity sets so duplicate persisted links remain visible.
fn assert_edge_occurrences(
    label: &str,
    actual: &[LocalId],
    target: &LocalId,
    expectation: EdgeExpectation,
) {
    let count = actual.iter().filter(|id| *id == target).count();
    let matches = match expectation {
        EdgeExpectation::ExactlyOnce => count == 1,
        EdgeExpectation::Contains => count > 0,
        EdgeExpectation::Absent => count == 0,
    };
    assert!(matches,
        "persisted edge {label}: expected {expectation:?}, found {count} target occurrences in {actual:?}");
}

/// Exact unordered identities, with duplicate expectations rejected rather than normalized away.
fn assert_exact_relationship_ids(
    label: &str,
    mut actual: Vec<LocalId>,
    mut expected: Vec<LocalId>,
) {
    actual.sort_by(|left, right| left.0.cmp(&right.0));
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        expected.windows(2).all(|pair| pair[0] != pair[1]),
        "persisted lineage {label}: duplicate declared identities {expected:?}"
    );
    assert_eq!(
        actual, expected,
        "persisted lineage {label}: expected exact identities without duplicates"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_lineage_accepts_unordered_identities_and_empty_roots() {
        assert_exact_relationship_ids(
            "branch successors",
            vec![LocalId(vec![2]), LocalId(vec![1])],
            vec![LocalId(vec![1]), LocalId(vec![2])],
        );
        assert_exact_relationship_ids("root predecessors", vec![], vec![]);
    }

    #[test]
    #[should_panic(expected = "expected exact identities without duplicates")]
    fn exact_lineage_rejects_extra_identity() {
        assert_exact_relationship_ids(
            "successor predecessors",
            vec![LocalId(vec![1]), LocalId(vec![2])],
            vec![LocalId(vec![2])],
        );
    }

    #[test]
    #[should_panic(expected = "expected exact identities without duplicates")]
    fn exact_lineage_rejects_duplicate_persisted_identity() {
        assert_exact_relationship_ids(
            "successor predecessors",
            vec![LocalId(vec![1]), LocalId(vec![1])],
            vec![LocalId(vec![1])],
        );
    }

    #[test]
    #[should_panic(expected = "duplicate declared identities")]
    fn exact_lineage_rejects_duplicate_declarations() {
        assert_exact_relationship_ids(
            "successor predecessors",
            vec![LocalId(vec![1]), LocalId(vec![1])],
            vec![LocalId(vec![1]), LocalId(vec![1])],
        );
    }

    #[test]
    fn edge_presence_and_absence_check_target_identity() {
        let ids = [LocalId(vec![1]), LocalId(vec![1])];
        assert_edge_occurrences("forward", &ids, &LocalId(vec![1]), EdgeExpectation::Contains);
        assert_edge_occurrences("forward", &ids, &LocalId(vec![2]), EdgeExpectation::Absent);
        assert_edge_occurrences(
            "inverse",
            &ids[..1],
            &LocalId(vec![1]),
            EdgeExpectation::ExactlyOnce,
        );
    }

    #[test]
    #[should_panic(expected = "found 2 target occurrences")]
    fn exactly_once_rejects_duplicate_edge() {
        assert_edge_occurrences(
            "forward",
            &[LocalId(vec![1]), LocalId(vec![1])],
            &LocalId(vec![1]),
            EdgeExpectation::ExactlyOnce,
        );
    }
}
