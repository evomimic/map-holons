//! Runtime registry and lookup utilities for realized references.
//!
//! `ExecutionHolons` maps a fixture token’s `TemporaryId` to the runtime
//! handle produced by a step (e.g., a newly staged holon). Executors
//! record new realizations here, and later steps *look them up* to obtain
//! the handles they need to call dances.
//!
//! ## Key points
//! - **Record**: after a step realizes something, wrap it in a
//!   `ResolvedTestReference` and store it under the source token’s `TemporaryId`.
//! - **Lookup (inputs)**: executors use `lookup_holon_reference(_vec)` to turn
//!   tokens into live `HolonReference`s *without* touching the Nursery or DHT.
//! - **Invariant**: Saved ≙ Staged(Committed(LocalId)) is enforced during lookup.
//! - **Most recent wins**: re-recording the same `TemporaryId` overwrites the prior entry.

use std::collections::BTreeMap;

use crate::harness::execution_support::ResolvedTestReference;
use crate::harness::fixtures_support::{ExpectedState, TestReference};
use core_types::{LocalId, TemporaryId};
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;

use super::ResultingReference;

#[derive(Clone, Debug, Default)]
pub struct ExecutionHolons {
    pub by_temporary_id: BTreeMap<TemporaryId, ResolvedTestReference>,
}

impl ExecutionHolons {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // -------------------------------------------------------------------------
    // Recording (Realization → Registry)
    // -------------------------------------------------------------------------

    /// Record a realized result using a fully-built `ResolvedTestReference`.
    ///
    /// Overwrites any previous entry for the same `TemporaryId` (most recent wins).
    pub fn record_resolved(&mut self, resolved: ResolvedTestReference) {
        self.by_temporary_id.insert(resolved.source_token.temporary_id(), resolved);
    }

    /// Convenience: construct and record from a source token + resulting handle.
    ///
    /// Returns a clone of the `ResolvedTestReference` just stored.
    pub fn record_from_parts(
        &mut self,
        source_token: TestReference,
        resulting_reference: ResultingReference,
    ) -> ResolvedTestReference {
        let resolved =
            ResolvedTestReference::from_reference_parts(source_token, resulting_reference);
        self.record_resolved(resolved.clone());
        resolved
    }

    // -------------------------------------------------------------------------
    // Lookup (Tokens → HolonReference)
    // -------------------------------------------------------------------------

    /// Turn a fixture token into the **current** runtime `HolonReference` to use as executor input.
    ///
    /// Lookup strategy:
    /// - `ExpectedState::Transient`  → return `HolonReference::Transient(token.transient().clone())`.
    /// - `ExpectedState::Staged`     → must find a recorded `StagedReference` **not committed**.
    /// - `ExpectedState::Saved`      → must find a recorded `StagedReference` **committed**.
    /// - `ExpectedState::Abandoned`  → must find a recorded `StagedReference` **abandoned**.
    /// - `ExpectedState::Deleted`  → return Error, caller should use `get_resulting_reference_for` instead
    ///
    /// No Nursery/DHT fallback is performed. Missing entries are treated as authoring/ordering errors.
    pub fn lookup_holon_reference(
        &self,
        context: &dyn HolonsContextBehavior,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        let expected_state = token.expected_state();

        match expected_state {
            ExpectedState::Deleted => Err(HolonError::InvalidParameter("Holon marked as deleted, to get ResultingReference, use get_resulting_reference_for function instead".to_string())),
            ExpectedState::Transient => Ok(HolonReference::Transient(token.transient().clone())),
            expected_state => {
                let resolved = self
                    .by_temporary_id
                    .get(&token.temporary_id())
                    .ok_or_else(|| HolonError::InvalidHolonReference(format!(
                        "ExecutionHolons::lookup: no realization recorded for TemporaryId {:?} (expected {:?})",
                        token.temporary_id(),
                        token.expected_state()
                    )))?;
                let holon_reference = &resolved.resulting_reference.get_holon_reference()?;
                match (expected_state, holon_reference) {
                    (
                        ExpectedState::Staged | ExpectedState::Abandoned,
                        HolonReference::Staged(staged_reference),
                    ) => {
                        if !staged_reference
                            .is_in_state(context, StagedState::Committed(LocalId(Vec::new())))?
                        {
                            Ok(HolonReference::Staged(staged_reference.clone()))
                        } else {
                            Err(HolonError::InvalidHolonReference(
                                    format!("ExecutionHolons::lookup for: {:?}, expected STAGED (not committed) got StagedReference but in StagedState::Committed", expected_state),
                                ))
                        }
                    }
                    (ExpectedState::Staged | ExpectedState::Abandoned, other) => {
                        Err(HolonError::InvalidHolonReference(format!(
                            "ExecutionHolons::lookup for: {:?}, expected STAGED, got: {:?}",
                            expected_state, other
                        )))
                    }
                    (ExpectedState::Saved, HolonReference::Smart(smart_reference)) => {
                        Ok(HolonReference::Smart(smart_reference.clone()))
                    }
                    (ExpectedState::Saved, other) => Err(HolonError::InvalidHolonReference(
                        format!("ExecutionHolons::lookup: expected SAVED, got: {:?}", other),
                    )),
                    (ExpectedState::Transient, _) => unreachable!("handled on first match arm"),
                    (ExpectedState::Deleted, _) => unreachable!("handled on first match arm"),
                }
            }
        }
    }

    /// Batch variant of `lookup_holon_reference`.
    pub fn lookup_holon_references(
        &self,
        context: &dyn HolonsContextBehavior,
        tokens: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        let mut references = Vec::new();
        for token in tokens {
            let reference = self.lookup_holon_reference(context, token)?;
            references.push(reference);
        }
        Ok(references)
    }

    // -------------------------------------------------------------------------
    // Introspection helpers
    // -------------------------------------------------------------------------

    /// Lookup the full resolved entry by source token’s `TemporaryId`.
    pub fn get_resolved(&self, temporary_id: &TemporaryId) -> Option<&ResolvedTestReference> {
        self.by_temporary_id.get(temporary_id)
    }

    /// Directly fetch the **resulting** `HolonReference` for a token, if recorded.
    ///
    /// Use `lookup_holon_reference` if you also need expected-state validation.
    pub fn get_resulting_reference_for(&self, token: &TestReference) -> Option<ResultingReference> {
        self.by_temporary_id.get(&token.temporary_id()).map(|r| r.resulting_reference.clone())
    }

    /// True if no realized entries have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.by_temporary_id.is_empty()
    }

    /// Iterate over all resolved entries (read-only).
    pub fn iter(&self) -> impl Iterator<Item = (&TemporaryId, &ResolvedTestReference)> {
        self.by_temporary_id.iter()
    }

    /// Number of realized entries currently tracked.
    pub fn len(&self) -> usize {
        self.by_temporary_id.len()
    }
}
