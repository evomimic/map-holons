//! Pure identity and occurrence assertions for persisted graph verification.

use crate::harness::test_case::EdgeExpectation;
use integrity_core_types::LocalId;
use pretty_assertions::assert_eq;

/// Counts occurrences rather than identity sets so duplicate persisted links remain visible.
pub fn assert_edge_occurrences(
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
///
/// Serves lineage and ordinary relationships alike; an empty `expected` asserts an
/// empty persisted collection rather than skipping the comparison.
pub fn assert_exact_relationship_ids(
    label: &str,
    mut actual: Vec<LocalId>,
    mut expected: Vec<LocalId>,
) {
    actual.sort_by(|left, right| left.0.cmp(&right.0));
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        expected.windows(2).all(|pair| pair[0] != pair[1]),
        "persisted relationship {label}: duplicate declared identities {expected:?}"
    );
    assert_eq!(
        actual, expected,
        "persisted relationship {label}: expected exact identities without duplicates"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_relationship_accepts_unordered_identities_and_empty_collections() {
        assert_exact_relationship_ids(
            "branch successors",
            vec![LocalId(vec![2]), LocalId(vec![1])],
            vec![LocalId(vec![1]), LocalId(vec![2])],
        );
        assert_exact_relationship_ids("root predecessors", vec![], vec![]);
    }

    #[test]
    #[should_panic(expected = "expected exact identities without duplicates")]
    fn exact_relationship_rejects_extra_identity() {
        assert_exact_relationship_ids(
            "successor predecessors",
            vec![LocalId(vec![1]), LocalId(vec![2])],
            vec![LocalId(vec![2])],
        );
    }

    #[test]
    #[should_panic(expected = "expected exact identities without duplicates")]
    fn exact_relationship_rejects_target_when_none_declared() {
        // An empty declaration is an assertion of emptiness, not an absent expectation.
        assert_exact_relationship_ids("references property", vec![LocalId(vec![9])], vec![]);
    }

    #[test]
    #[should_panic(expected = "expected exact identities without duplicates")]
    fn exact_relationship_rejects_duplicate_persisted_identity() {
        assert_exact_relationship_ids(
            "successor predecessors",
            vec![LocalId(vec![1]), LocalId(vec![1])],
            vec![LocalId(vec![1])],
        );
    }

    #[test]
    #[should_panic(expected = "duplicate declared identities")]
    fn exact_relationship_rejects_duplicate_declarations() {
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
