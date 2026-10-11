//! `Distinct` — identity-based deduplication for QueryCore (QRY4b).
//!
//! `Distinct` is a concrete, parameter-free `QueryExpression` kind: it declares
//! no properties or relationships, so there is no argument to validate. It
//! keeps the first occurrence of each identity, returns that occurrence's
//! original reference, and preserves the relative input order of the retained
//! occurrences: `[A, B, A, C, B]` becomes `[A, B, C]`.
//!
//! Identity is exactly `HolonReference` equality, owned by the reference layer:
//! saved references compare by `HolonId` (and owning Space Manager instance for
//! local ids), staged and transient references by transaction and temporary id,
//! and references in different phases never match. Property values, cached
//! hints, keys, and diagnostic strings never participate. Detection hashes with
//! `HolonReference`'s `Hash`, which agrees with that equality, so it makes the
//! same decisions as a pairwise comparison.

use std::collections::HashSet;
use std::hash::Hash;

/// `Distinct`: retains the first occurrence of each member in input order.
pub(crate) fn distinct<T: Eq + Hash + Clone>(members: &[T]) -> Vec<T> {
    let mut seen = HashSet::with_capacity(members.len());
    members.iter().filter(|member| seen.insert(*member)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_singleton_inputs_are_unchanged() {
        assert!(distinct::<&str>(&[]).is_empty());
        assert_eq!(distinct(&["a"]), vec!["a"]);
    }

    #[test]
    fn all_unique_input_is_unchanged_in_order() {
        assert_eq!(distinct(&["c", "a", "b"]), vec!["c", "a", "b"]);
    }

    #[test]
    fn adjacent_and_separated_repeats_keep_the_first_occurrence() {
        assert_eq!(distinct(&["a", "a", "b"]), vec!["a", "b"]);
        assert_eq!(distinct(&["A", "B", "A", "C", "B"]), vec!["A", "B", "C"]);
    }

    #[test]
    fn all_identical_input_keeps_one_occurrence() {
        assert_eq!(distinct(&["x", "x", "x", "x"]), vec!["x"]);
    }
}
