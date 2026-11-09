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

/// Composes fixture-authored changes on a named persisted baseline. Identical sibling
/// additions are idempotent; unrelated baseline members and duplicate evidence survive.
pub fn apply_fixture_membership_change(
    members: &mut Vec<LocalId>,
    before: &[LocalId],
    after: &[LocalId],
) {
    let touched: std::collections::HashSet<_> = before.iter().chain(after).cloned().collect();
    for id in touched {
        let prior = before.iter().filter(|value| **value == id).count();
        let desired = after.iter().filter(|value| **value == id).count();
        let current = members.iter().filter(|value| **value == id).count();
        let count = if desired > prior {
            current.max(desired)
        } else if desired < prior {
            current.min(desired)
        } else {
            continue;
        };
        members.retain(|value| *value != id);
        members.extend(std::iter::repeat(id).take(count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    #[should_panic(expected = "found 0 target occurrences")]
    fn exactly_once_rejects_missing_inverse() {
        assert_edge_occurrences("inverse", &[], &LocalId(vec![1]), EdgeExpectation::ExactlyOnce);
    }

    #[test]
    fn fixture_changes_compose_siblings_without_collapsing_unrelated_evidence() {
        let a = LocalId(vec![1]);
        let b = LocalId(vec![2]);
        let unrelated = LocalId(vec![9]);
        let mut members = vec![unrelated.clone(), unrelated.clone()];
        apply_fixture_membership_change(&mut members, &[], &[a.clone()]);
        apply_fixture_membership_change(&mut members, &[], &[a.clone(), b.clone()]);
        assert_eq!(members.iter().filter(|id| **id == a).count(), 1);
        assert_eq!(members.iter().filter(|id| **id == b).count(), 1);
        assert_eq!(members.iter().filter(|id| **id == unrelated).count(), 2);
        apply_fixture_membership_change(&mut members, &[a.clone()], &[]);
        assert!(!members.contains(&a));
        assert!(members.contains(&b));
    }

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
