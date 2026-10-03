//! Commit observations and diagnostics independent of fixture declarations.
//!
//! These helpers interpret exposed lifecycle state and saved identities. They do not
//! reproduce guest mutation policy or infer expectations from a Commit response.

use crate::ExpectedDisposition;
use core_types::LocalId;
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;
use integrity_core_types::HolonErrorKind;
use std::{collections::HashMap, fmt};

/// Retained candidate identity and lifecycle observations for one Commit attempt.
#[derive(Clone, Debug)]
pub struct ObservedCandidate {
    pub identity: String,
    pub key: Option<MapString>,
    pub versioned_source_id: Option<LocalId>,
    pub staged_state: StagedState,
    /// Identity of the corresponding `SavedHolons` member, if one was matched.
    pub saved_holons_id: Option<LocalId>,
}

/// Persistence disposition determined from lifecycle state and response membership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedDisposition {
    NewRoot,
    NoAction,
    GraphOnly,
    NewVersion,
    /// The observations do not describe a supported saved or no-action outcome.
    /// In particular, a Pass 1 failure does not establish whether a node was written.
    Unsupported {
        reason: String,
    },
}

impl ObservedDisposition {
    /// Returns the declarable disposition, or `None` for unsupported observations.
    pub fn as_expected(&self) -> Option<ExpectedDisposition> {
        match self {
            Self::NewRoot => Some(ExpectedDisposition::NewRoot),
            Self::NoAction => Some(ExpectedDisposition::NoAction),
            Self::GraphOnly => Some(ExpectedDisposition::GraphOnly),
            Self::NewVersion => Some(ExpectedDisposition::NewVersion),
            Self::Unsupported { .. } => None,
        }
    }
}

impl fmt::Display for ObservedDisposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewRoot => write!(formatter, "NewRoot"),
            Self::NoAction => write!(formatter, "NoAction"),
            Self::GraphOnly => write!(formatter, "GraphOnly"),
            Self::NewVersion => write!(formatter, "NewVersion"),
            Self::Unsupported { reason } => write!(formatter, "Unsupported: {reason}"),
        }
    }
}

/// Classifies one live Pass 1 candidate without consulting its declaration.
/// Missing saved membership is no action only for an unchanged update with a source.
pub fn classify(observed: &ObservedCandidate) -> ObservedDisposition {
    match (&observed.staged_state, &observed.saved_holons_id) {
        (StagedState::Committed(committed_id), Some(saved_id)) if committed_id == saved_id => {
            match &observed.versioned_source_id {
                None => ObservedDisposition::NewRoot,
                Some(source_id) if source_id == committed_id => ObservedDisposition::GraphOnly,
                Some(_) => ObservedDisposition::NewVersion,
            }
        }
        (StagedState::ForUpdate, None) if observed.versioned_source_id.is_some() => {
            ObservedDisposition::NoAction
        }
        (
            StagedState::ForCreate
            | StagedState::ForUpdateGraphOnly
            | StagedState::ForUpdateNewVersion,
            None,
        ) => ObservedDisposition::Unsupported {
            reason: format!(
                "Pass 1 did not reach a Saved outcome: state {}, absent from SavedHolons; this does not establish whether a node was written",
                observed.staged_state
            ),
        },
        _ => ObservedDisposition::Unsupported {
            reason: format!(
                "inconsistent candidate observations: state {}, source {:?}, SavedHolons identity {:?}",
                observed.staged_state, observed.versioned_source_id, observed.saved_holons_id
            ),
        },
    }
}

/// Returns candidate indices in saved-result order, matching only committed identities.
/// Keys and declarations never participate. Each result must have exactly one claimant,
/// and duplicate result identities are rejected rather than recorded twice.
/// On success, replaces all `saved_holons_id` memberships with the matched identities,
/// clearing unmatched candidates. A correspondence failure leaves observations unchanged.
pub fn match_saved_holons(
    candidates: &mut [ObservedCandidate],
    saved_ids: &[LocalId],
) -> Result<Vec<usize>, String> {
    let mut matched = Vec::new();
    for saved_id in saved_ids {
        let claimants: Vec<_> = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                matches!(&candidate.staged_state, StagedState::Committed(id) if id == saved_id)
            })
            .collect();
        match claimants.as_slice() {
            [(index, candidate)] => {
                if matched.contains(index) {
                    return Err(format!(
                        "Duplicate SavedHolons identity {saved_id:?} for candidate {} (key {:?})",
                        candidate.identity, candidate.key
                    ));
                }
                matched.push(*index);
            }
            _ => {
                let labels: Vec<_> = claimants
                    .iter()
                    .map(|(_, candidate)| {
                        format!("{} (key {:?})", candidate.identity, candidate.key)
                    })
                    .collect();
                let correspondence = if claimants.is_empty() { "Unmatched" } else { "Ambiguous" };
                return Err(format!(
                    "{correspondence} SavedHolons identity {saved_id:?}: expected exactly one committed candidate, found {}; claimants: [{}]",
                    claimants.len(), labels.join(", ")
                ));
            }
        }
    }
    for candidate in candidates.iter_mut() {
        candidate.saved_holons_id = None;
    }
    for (saved_id, index) in saved_ids.iter().zip(&matched) {
        candidates[*index].saved_holons_id = Some(saved_id.clone());
    }
    Ok(matched)
}

/// Reports the entire declaration/observation table when any row differs or is unsupported.
/// Includes matching rows to show the Commit workset alongside unexpected dispositions.
pub fn disposition_report(
    rows: &[(String, Option<MapString>, ExpectedDisposition, ObservedDisposition)],
) -> Option<String> {
    if rows.iter().all(|(_, _, expected, observed)| Some(*expected) == observed.as_expected()) {
        return None;
    }
    let mut report = String::from("Commit disposition mismatch (declared versus observed):");
    for (identity, key, expected, observed) in rows {
        report.push_str(&format!(
            "\nCandidate {identity} (key {key:?}): declared {expected}, observed {observed}"
        ));
    }
    Some(report)
}

fn error_counts(kinds: &[HolonErrorKind]) -> HashMap<HolonErrorKind, usize> {
    let mut counts = HashMap::new();
    for kind in kinds {
        *counts.entry(*kind).or_default() += 1;
    }
    counts
}

/// Multiset difference against an append-only pre-attempt baseline.
/// Preserves post-attempt order and counts a repeated kind as a new occurrence.
pub fn new_error_occurrences(
    before: &[HolonErrorKind],
    after: &[HolonErrorKind],
) -> Vec<HolonErrorKind> {
    let mut remaining = error_counts(before);
    let mut new = Vec::new();
    for kind in after {
        match remaining.get_mut(kind) {
            Some(count) if *count > 0 => *count -= 1,
            _ => new.push(*kind),
        }
    }
    new
}

fn sorted_error_kinds(kinds: &[HolonErrorKind]) -> Vec<HolonErrorKind> {
    let mut sorted = kinds.to_vec();
    sorted.sort_by_key(|kind| format!("{kind:?}"));
    sorted
}

/// Reports mismatched new operational errors by kind and multiplicity, including
/// unexpected errors on candidates or retained retry participants declaring none.
/// Includes only mismatching rows because error deltas are independent per-candidate facts.
pub fn error_delta_report(
    rows: &[(String, Vec<HolonErrorKind>, Vec<HolonErrorKind>)],
) -> Option<String> {
    let mismatches: Vec<_> = rows
        .iter()
        .filter(|(_, expected, observed)| error_counts(expected) != error_counts(observed))
        .collect();
    if mismatches.is_empty() {
        return None;
    }
    let mut report = String::from("Commit new operational-error occurrence mismatch:");
    for (identity, expected, observed) in mismatches {
        report.push_str(&format!(
            "\nCandidate {identity}: declared {:?}, observed {:?}",
            sorted_error_kinds(expected),
            sorted_error_kinds(observed)
        ));
    }
    Some(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> LocalId {
        LocalId(vec![byte; 39])
    }

    fn candidate(
        identity: &str,
        source: Option<LocalId>,
        state: StagedState,
        saved: Option<LocalId>,
    ) -> ObservedCandidate {
        ObservedCandidate {
            identity: identity.into(),
            key: Some(MapString("shared-key".into())),
            versioned_source_id: source,
            staged_state: state,
            saved_holons_id: saved,
        }
    }

    #[test]
    fn classifies_each_supported_disposition_from_observations() {
        for (source, state, saved, expected) in [
            (None, StagedState::Committed(id(1)), Some(id(1)), ObservedDisposition::NewRoot),
            (Some(id(1)), StagedState::ForUpdate, None, ObservedDisposition::NoAction),
            (
                Some(id(1)),
                StagedState::Committed(id(1)),
                Some(id(1)),
                ObservedDisposition::GraphOnly,
            ),
            (
                Some(id(1)),
                StagedState::Committed(id(2)),
                Some(id(2)),
                ObservedDisposition::NewVersion,
            ),
        ] {
            assert_eq!(classify(&candidate("candidate", source, state, saved)), expected);
        }
    }

    #[test]
    fn pass_one_failures_are_unsupported_without_inferring_node_persistence() {
        for state in [
            StagedState::ForCreate,
            StagedState::ForUpdateGraphOnly,
            StagedState::ForUpdateNewVersion,
        ] {
            let observed = candidate("failed-candidate", Some(id(1)), state.clone(), None);
            let disposition = classify(&observed);
            let ObservedDisposition::Unsupported { reason } = &disposition else {
                panic!("must be unsupported")
            };
            assert!(reason.contains(&state.to_string()));
            assert!(reason.contains("does not establish whether a node was written"));
            let report = disposition_report(&[(
                observed.identity,
                observed.key,
                ExpectedDisposition::NewVersion,
                disposition,
            )])
            .unwrap();
            assert!(report.contains("failed-candidate"));
            assert!(report.contains("observed Unsupported"));
            assert!(!report.contains("NoAction"));
        }
    }

    #[test]
    fn inconsistent_states_and_response_membership_are_unsupported() {
        for (source, state, saved) in [
            (None, StagedState::ForUpdate, None),
            (None, StagedState::Committed(id(1)), None),
            (Some(id(1)), StagedState::Abandoned, None),
            (None, StagedState::ForCreate, Some(id(1))),
            (Some(id(1)), StagedState::ForUpdate, Some(id(1))),
            (None, StagedState::Committed(id(1)), Some(id(2))),
        ] {
            assert!(matches!(
                classify(&candidate("inconsistent", source, state, saved)),
                ObservedDisposition::Unsupported { .. }
            ));
        }
    }

    #[test]
    fn stale_declaration_reports_both_dispositions_and_all_candidates() {
        let rows = vec![
            (
                "property-mutated".into(),
                Some(MapString("book".into())),
                ExpectedDisposition::GraphOnly,
                ObservedDisposition::NewVersion,
            ),
            (
                "unchanged".into(),
                None,
                ExpectedDisposition::NoAction,
                ObservedDisposition::NoAction,
            ),
        ];
        let report = disposition_report(&rows).unwrap();
        assert!(report.contains("property-mutated"));
        assert!(report.contains("book"));
        assert!(report.contains("declared GraphOnly, observed NewVersion"));
        assert!(report.contains("unchanged"));
    }

    #[test]
    fn matching_dispositions_and_empty_worksets_have_no_report() {
        assert!(disposition_report(&[]).is_none());
        let rows = vec![
            ("root".into(), None, ExpectedDisposition::NewRoot, ObservedDisposition::NewRoot),
            (
                "unchanged".into(),
                None,
                ExpectedDisposition::NoAction,
                ObservedDisposition::NoAction,
            ),
            ("graph".into(), None, ExpectedDisposition::GraphOnly, ObservedDisposition::GraphOnly),
            (
                "version".into(),
                None,
                ExpectedDisposition::NewVersion,
                ObservedDisposition::NewVersion,
            ),
        ];
        assert!(disposition_report(&rows).is_none());
    }

    #[test]
    fn same_key_versions_match_by_identity_in_response_order() {
        let mut candidates = vec![
            candidate("version-b", Some(id(1)), StagedState::Committed(id(2)), None),
            candidate("version-c", Some(id(1)), StagedState::Committed(id(3)), None),
            candidate("unchanged", Some(id(4)), StagedState::ForUpdate, None),
        ];
        let saved = [id(3), id(2)];
        let matches = match_saved_holons(&mut candidates, &saved).unwrap();
        assert_eq!(matches, vec![1, 0]);
        assert_eq!(candidates[0].saved_holons_id, Some(id(2)));
        assert_eq!(candidates[1].saved_holons_id, Some(id(3)));
        assert_eq!(candidates[2].saved_holons_id, None);
        assert_eq!(classify(&candidates[0]), ObservedDisposition::NewVersion);
        assert_eq!(classify(&candidates[1]), ObservedDisposition::NewVersion);
        assert_eq!(classify(&candidates[2]), ObservedDisposition::NoAction);
    }

    #[test]
    fn ambiguous_identity_names_every_claimant() {
        let mut candidates = vec![
            candidate("first", Some(id(1)), StagedState::Committed(id(1)), None),
            candidate("second", Some(id(1)), StagedState::Committed(id(1)), None),
        ];
        let report = match_saved_holons(&mut candidates, &[id(1)]).unwrap_err();
        for expected in ["Ambiguous", "first", "second", "shared-key", "found 2"] {
            assert!(report.contains(expected), "{report}");
        }
        assert!(report.contains(&format!("{:?}", id(1))));
    }

    #[test]
    fn unmatched_and_duplicate_saved_identities_fail_clearly() {
        let mut candidates = vec![candidate("root", None, StagedState::Committed(id(1)), None)];
        assert!(match_saved_holons(&mut candidates, &[id(2)]).unwrap_err().contains("Unmatched"));
        let duplicate = match_saved_holons(&mut candidates, &[id(1), id(1)]).unwrap_err();
        assert!(duplicate.contains("Duplicate"));
        assert!(duplicate.contains("root"));
        assert_eq!(match_saved_holons(&mut [], &[]).unwrap(), Vec::<usize>::new());
    }

    #[test]
    fn successful_matching_replaces_stale_membership_and_clears_unmatched_candidates() {
        let mut candidates = vec![
            candidate("root", None, StagedState::Committed(id(1)), Some(id(9))),
            candidate("unchanged", Some(id(2)), StagedState::ForUpdate, Some(id(9))),
        ];
        assert_eq!(match_saved_holons(&mut candidates, &[id(1)]).unwrap(), vec![0]);
        assert_eq!(candidates[0].saved_holons_id, Some(id(1)));
        assert_eq!(classify(&candidates[0]), ObservedDisposition::NewRoot);
        assert_eq!(candidates[1].saved_holons_id, None);
        assert_eq!(classify(&candidates[1]), ObservedDisposition::NoAction);
        assert!(match_saved_holons(&mut candidates, &[]).unwrap().is_empty());
        assert!(candidates.iter().all(|candidate| candidate.saved_holons_id.is_none()));
    }

    #[test]
    fn failed_correspondence_leaves_all_memberships_unchanged() {
        let mut candidates = vec![
            candidate("first", None, StagedState::Committed(id(1)), Some(id(8))),
            candidate("second", None, StagedState::Committed(id(2)), Some(id(9))),
        ];
        for saved_ids in [vec![id(1), id(3)], vec![id(1), id(1)]] {
            assert!(match_saved_holons(&mut candidates, &saved_ids).is_err());
            assert_eq!(candidates[0].saved_holons_id, Some(id(8)));
            assert_eq!(candidates[1].saved_holons_id, Some(id(9)));
        }
    }

    #[test]
    fn repeated_retry_error_is_a_new_occurrence() {
        let before = [HolonErrorKind::CommitFailure];
        let after = [HolonErrorKind::CommitFailure, HolonErrorKind::CommitFailure];
        let delta = new_error_occurrences(&before, &after);
        assert_eq!(delta, vec![HolonErrorKind::CommitFailure]);
        assert!(error_delta_report(&[(
            "retry-participant".into(),
            vec![HolonErrorKind::CommitFailure],
            delta
        )])
        .is_none());
    }

    #[test]
    fn error_difference_preserves_kind_and_multiplicity() {
        use HolonErrorKind::{CommitFailure, PvlViolation};
        assert_eq!(
            new_error_occurrences(
                &[CommitFailure, PvlViolation],
                &[PvlViolation, CommitFailure, CommitFailure, PvlViolation, PvlViolation]
            ),
            vec![CommitFailure, PvlViolation, PvlViolation]
        );
        assert!(new_error_occurrences(&[CommitFailure], &[CommitFailure]).is_empty());
        assert_eq!(
            new_error_occurrences(&[], &[CommitFailure, CommitFailure]),
            vec![CommitFailure, CommitFailure]
        );
    }

    #[test]
    fn error_reports_compare_multisets_and_sort_diagnostics() {
        use HolonErrorKind::{CommitFailure, PvlViolation};
        assert!(error_delta_report(&[]).is_none());
        assert!(error_delta_report(&[(
            "ordered".into(),
            vec![PvlViolation, CommitFailure],
            vec![CommitFailure, PvlViolation]
        )])
        .is_none());
        let rows = vec![
            ("wrong-kind".into(), vec![CommitFailure], vec![PvlViolation]),
            ("wrong-count".into(), vec![CommitFailure], vec![CommitFailure, CommitFailure]),
            ("unexpected".into(), vec![], vec![PvlViolation, CommitFailure]),
            ("missing".into(), vec![CommitFailure], vec![]),
        ];
        let report = error_delta_report(&rows).unwrap();
        for name in ["wrong-kind", "wrong-count", "unexpected", "missing"] {
            assert!(report.contains(name));
        }
        assert!(report.contains("observed [CommitFailure, PvlViolation]"));
        assert!(report.contains("observed [CommitFailure, CommitFailure]"));
    }
}
