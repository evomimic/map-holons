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
