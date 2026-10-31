//! Scoped cache observations for diagnostics. No content, cache policy, or outcomes are changed.
use core_types::{HolonId, RelationshipName};
use std::{cell::RefCell, collections::HashMap};

type ReadKey = (HolonId, Option<RelationshipName>, &'static str);
thread_local! {
    static READS: RefCell<Option<HashMap<ReadKey, usize>>> = const { RefCell::new(None) };
}

/// Cache requests grouped by exact source identity, optional relationship name, and outcome.
/// A missing relationship name denotes saved-node content access.
pub struct CacheReadObservation {
    /// Exact saved source identity.
    pub source: HolonId,
    /// Membership name, or None for saved-node resolution.
    pub relationship: Option<RelationshipName>,
    /// Cache hit or the missing, expired, or forced-fresh reason.
    pub outcome: &'static str,
    /// Number of requests with this source, name, and outcome.
    pub requests: usize,
}

/// Aggregate logical demand and cache outcomes, keeping content and membership requests distinct.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CacheReadSummary {
    /// All recorded cache requests, including content and membership reads.
    pub requests: usize,
    /// Unique exact source/relationship pairs, independent of outcome.
    pub distinct_memberships: usize,
    /// Requests for relationship memberships, including requested-empty sets.
    pub membership_requests: usize,
    /// Requests resolving saved-node content.
    pub content_requests: usize,
    /// Requests served by an unexpired cache entry.
    pub hits: usize,
    /// Missing, expired, forced-fresh, or otherwise unsatisfied cache requests.
    pub misses: usize,
}
impl CacheReadSummary {
    /// Summarize already recorded observations without resolving content or querying relationships.
    pub fn from_observations(observations: &[CacheReadObservation]) -> Self {
        let mut summary = Self::default();
        let mut memberships = std::collections::HashSet::new();
        for read in observations {
            summary.requests += read.requests;
            if let Some(name) = &read.relationship {
                summary.membership_requests += read.requests;
                memberships.insert((&read.source, name));
            } else {
                summary.content_requests += read.requests;
            }
            if read.outcome == "hit" {
                summary.hits += read.requests;
            } else {
                summary.misses += read.requests;
            }
        }
        summary.distinct_memberships = memberships.len();
        summary
    }
}

/// Records requests only within one explicitly enabled diagnostic scope.
/// Nested scopes are inactive so they cannot replace an outer scope's evidence.
pub struct CacheReadProbe(bool);
impl CacheReadProbe {
    /// Begin observation on this execution thread when no enclosing probe is active.
    pub fn start(enabled: bool) -> Self {
        let active = enabled
            && READS.with(|reads| {
                let mut reads = reads.borrow_mut();
                if reads.is_some() {
                    return false;
                }
                *reads = Some(HashMap::new());
                true
            });
        Self(active)
    }
    /// Close the scope and return its grouped observations.
    pub fn finish(mut self) -> Vec<CacheReadObservation> {
        if !self.0 {
            return Vec::new();
        }
        self.0 = false;
        READS
            .with(|reads| reads.borrow_mut().take().unwrap_or_default())
            .into_iter()
            .map(|((source, relationship, outcome), requests)| CacheReadObservation {
                source,
                relationship,
                outcome,
                requests,
            })
            .collect()
    }
}
impl Drop for CacheReadProbe {
    fn drop(&mut self) {
        if self.0 {
            READS.with(|reads| *reads.borrow_mut() = None);
        }
    }
}
pub(super) fn record(
    source: &HolonId,
    relationship: Option<&RelationshipName>,
    outcome: &'static str,
) {
    READS.with(|reads| {
        if let Some(reads) = reads.borrow_mut().as_mut() {
            *reads.entry((source.clone(), relationship.cloned(), outcome)).or_default() += 1;
        }
    });
}

thread_local! {
    static DERIVATIONS: RefCell<Option<std::collections::BTreeMap<&'static str, usize>>> = const { RefCell::new(None) };
}

/// Clock-free work observations. Names identify actual execution boundaries, not cache misses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivationObservations(pub std::collections::BTreeMap<&'static str, usize>);

/// One explicit observation scope; nested probes cannot consume their parent's evidence.
pub struct DerivationProbe(bool);
impl DerivationProbe {
    /// Start a scope only when enabled and no outer scope owns the counters.
    pub fn start(enabled: bool) -> Self {
        Self(
            enabled
                && DERIVATIONS.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    if slot.is_some() {
                        return false;
                    }
                    *slot = Some(Default::default());
                    true
                }),
        )
    }
    /// Copy observations without closing the scope.
    pub fn snapshot(&self) -> DerivationObservations {
        DerivationObservations(if self.0 {
            DERIVATIONS.with(|slot| slot.borrow().clone().unwrap_or_default())
        } else {
            Default::default()
        })
    }
    /// Close the scope, including failed operations observed before an early return.
    pub fn finish(mut self) -> DerivationObservations {
        if !self.0 {
            return Default::default();
        }
        self.0 = false;
        DerivationObservations(
            DERIVATIONS.with(|slot| slot.borrow_mut().take().unwrap_or_default()),
        )
    }
}
impl Drop for DerivationProbe {
    fn drop(&mut self) {
        if self.0 {
            DERIVATIONS.with(|slot| *slot.borrow_mut() = None);
        }
    }
}

/// Record work only when an explicitly enabled probe is active. Never resolves content.
pub fn record_derivation(operation: &'static str, count: usize) {
    DERIVATIONS.with(|slot| {
        if let Some(counts) = slot.borrow_mut().as_mut() {
            *counts.entry(operation).or_default() += count;
        }
    });
}

/// Count an attempted operation and successful completion without changing its error path.
pub(crate) fn observe_attempt<T, E>(
    attempts: &'static str,
    successes: &'static str,
    action: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    record_derivation(attempts, 1);
    let result = action();
    if result.is_ok() {
        record_derivation(successes, 1);
    }
    result
}

#[cfg(test)]
mod derivation_tests {
    use super::*;
    #[test]
    fn summary_preserves_content_membership_and_cache_outcome_accounting() {
        use type_names::ToRelationshipName;
        let source = HolonId::Local(core_types::LocalId(vec![1; 39]));
        let name = type_names::CoreRelationshipTypeName::Extends.to_relationship_name();
        let observation = |relationship, outcome, requests| CacheReadObservation {
            source: source.clone(),
            relationship,
            outcome,
            requests,
        };
        let observations = vec![
            observation(None, "hit", 2),
            observation(None, "missing", 1),
            observation(Some(name.clone()), "hit", 3),
            observation(Some(name.clone()), "missing", 1),
            observation(Some(name.clone()), "expired", 2),
            observation(Some(name), "fresh", 1),
        ];
        assert_eq!(
            CacheReadSummary::from_observations(&observations),
            CacheReadSummary {
                requests: 10,
                content_requests: 3,
                membership_requests: 7,
                distinct_memberships: 1,
                hits: 5,
                misses: 5,
            }
        );
        assert_eq!(CacheReadSummary::from_observations(&[]), CacheReadSummary::default());
    }

    #[test]
    fn attempt_observation_counts_failures_without_claiming_success() {
        let probe = DerivationProbe::start(true);
        assert_eq!(
            observe_attempt("attempts", "successes", || Err::<(), _>("failure")),
            Err("failure")
        );
        assert_eq!(observe_attempt("attempts", "successes", || Ok::<_, ()>(7)), Ok(7));
        let counts = probe.finish().0;
        assert_eq!(counts["attempts"], 2);
        assert_eq!(counts["successes"], 1);
    }

    #[test]
    fn nested_disabled_snapshot_and_drop_preserve_scope_ownership() {
        let outer = DerivationProbe::start(true);
        record_derivation("attempt", 1);
        assert!(DerivationProbe::start(true).finish().0.is_empty());
        assert!(DerivationProbe::start(false).finish().0.is_empty());
        assert_eq!(outer.snapshot().0["attempt"], 1);
        record_derivation("attempt", 1);
        assert_eq!(outer.finish().0["attempt"], 2);
        {
            let _abandoned = DerivationProbe::start(true);
            record_derivation("stale", 1);
        }
        assert!(DerivationProbe::start(true).finish().0.is_empty());
    }
}
