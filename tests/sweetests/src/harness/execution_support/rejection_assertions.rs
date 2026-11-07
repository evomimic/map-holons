//! Bounded rejection assertions, independent of conductor setup and fixture authoring.

use holons_prelude::prelude::*;
use integrity_core_types::LocalId;
use std::collections::HashSet;

/// Rejects missing, additional, and duplicated candidate identities before Commit dispatch.
pub fn assert_rejection_workset(expected: &[String], actual: &[String]) {
    let expected_set: HashSet<_> = expected.iter().collect();
    let actual_set: HashSet<_> = actual.iter().collect();
    assert_eq!(expected.len(), expected_set.len(), "duplicate fixture rejection subject");
    assert_eq!(actual.len(), actual_set.len(), "duplicate runtime rejection candidate");
    assert_eq!(expected_set, actual_set, "fixture rejection workset differs from live candidates");
}

/// Only a real not-found result establishes saved-key absence; operational errors do not.
pub fn assert_rejection_key_absent(key: &MapString, result: Result<(), HolonError>) {
    match result {
        Err(HolonError::HolonNotFound(_)) => (),
        Ok(()) => panic!("rejection subject {key} is persisted"),
        Err(error) => panic!("cannot establish saved-key absence for {key}: {error:?}"),
    }
}

/// Compares occurrences, retaining duplicates that existed before the attempt.
pub fn assert_rejection_relationship_unchanged(
    label: &str,
    mut before: Vec<LocalId>,
    mut after: Vec<LocalId>,
) {
    before.sort_by(|a, b| a.0.cmp(&b.0));
    after.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(before, after, "rejection changed persisted relationship {label}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "workset differs")]
    fn undeclared_runtime_candidate_fails() {
        assert_rejection_workset(&["book".into()], &["book".into(), "person".into()]);
    }

    #[test]
    #[should_panic(expected = "workset differs")]
    fn missing_runtime_candidate_fails() {
        assert_rejection_workset(&["book".into(), "person".into()], &["book".into()]);
    }

    #[test]
    #[should_panic(expected = "is persisted")]
    fn leaked_saved_subject_fails() {
        assert_rejection_key_absent(&MapString("book".into()), Ok(()));
    }

    #[test]
    #[should_panic(expected = "cannot establish saved-key absence")]
    fn operational_error_is_not_absence() {
        assert_rejection_key_absent(
            &MapString("book".into()),
            Err(HolonError::Misc("offline".into())),
        );
    }

    #[test]
    #[should_panic(expected = "changed persisted relationship")]
    fn leaked_inverse_occurrence_fails() {
        assert_rejection_relationship_unchanged("Instances", vec![], vec![LocalId(vec![1])]);
    }

    #[test]
    #[should_panic(expected = "changed persisted relationship")]
    fn duplicated_inverse_occurrence_fails() {
        assert_rejection_relationship_unchanged(
            "Instances",
            vec![LocalId(vec![1])],
            vec![LocalId(vec![1]), LocalId(vec![1])],
        );
    }

    #[test]
    fn unchanged_observations_accept_order_and_preexisting_duplicates() {
        assert_rejection_workset(
            &["book".into(), "person".into()],
            &["person".into(), "book".into()],
        );
        assert_rejection_key_absent(
            &MapString("book".into()),
            Err(HolonError::HolonNotFound("book".into())),
        );
        assert_rejection_relationship_unchanged(
            "Instances",
            vec![LocalId(vec![1]), LocalId(vec![2]), LocalId(vec![1])],
            vec![LocalId(vec![2]), LocalId(vec![1]), LocalId(vec![1])],
        );
    }
}
