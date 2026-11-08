//! Attempt-scoped persisted verification inputs retained across relationship retries.

use crate::{ExpectedCommitRelationship, ResolvedCommitCandidate};
use integrity_core_types::LocalId;

/// Only named fixture memberships and the declared saved source are observed.
/// Baselines preserve unknown members of partial lookup stubs without inventing expectations.
#[derive(Clone, Debug)]
pub struct CommitVerificationBaseline {
    pub candidate: ResolvedCommitCandidate,
    pub lineage: Option<(Vec<LocalId>, Vec<LocalId>)>,
    pub relationships: Vec<(ExpectedCommitRelationship, Vec<LocalId>)>,
}

/// Incoming membership of a fixture-created identity is completely known: every
/// contributor must come from a declared forward write. Exact source versions remain
/// distinct even when inverse policy projects them onto one lineage identity.
pub type FixtureInverseContributors =
    std::collections::HashMap<(LocalId, String), std::collections::HashMap<LocalId, LocalId>>;
