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
use crate::harness::fixtures_support::{IntendedResolvedState, TestReference};
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
        self.by_temporary_id.insert(resolved.fixture_token.token_id().temporary_id(), resolved);
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
    /// - `IntendedResolvedState::Transient`  → return `HolonReference::Transient(token.token_id().clone())`.
    /// - `IntendedResolvedState::Staged`     → must find a recorded `StagedReference` **not committed**.
    /// - `IntendedResolvedState::Saved`      → must find a recorded `StagedReference` **committed**.
    /// - `IntendedResolvedState::Abandoned`  → must find a recorded `StagedReference` **abandoned**.
    /// - `IntendedResolvedState::Deleted`  → return Error
    ///
    /// No Nursery/DHT fallback is performed. Missing entries are treated as authoring/ordering errors.
    pub fn lookup_holon_reference(
        &self,
        context: &dyn HolonsContextBehavior,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        let intended_resolved_state = token.intended_resolved_state();

        match intended_resolved_state {
            IntendedResolvedState::Deleted => Err(HolonError::InvalidParameter(
                "Holon marked as deleted, there is no associated resolved HolonReference"
                    .to_string(),
            )),
            IntendedResolvedState::Transient => {
                Ok(HolonReference::Transient(token.token_id().clone()))
            }
            intended_resolved_state => {
                let resolved = self
                    .by_temporary_id
                    .get(&token.token_id().temporary_id())
                    .ok_or_else(|| HolonError::InvalidHolonReference(format!(
                        "ExecutionHolons::lookup: no realization recorded for TemporaryId {:?} (expected {:?})",
                        token.token_id(),
                        token.intended_resolved_state()
                    )))?;
                let holon_reference = &resolved.resulting_reference.get_holon_reference()?;
                match (intended_resolved_state, holon_reference) {
                    (
                        IntendedResolvedState::Staged | IntendedResolvedState::Abandoned,
                        HolonReference::Staged(staged_reference),
                    ) => {
                        if !staged_reference
                            .is_in_state(context, StagedState::Committed(LocalId(Vec::new())))?
                        {
                            Ok(HolonReference::Staged(staged_reference.clone()))
                        } else {
                            Err(HolonError::InvalidHolonReference(
                                    format!("ExecutionHolons::lookup for: {:?}, expected STAGED (not committed) got StagedReference but in StagedState::Committed", intended_resolved_state),
                                ))
                        }
                    }
                    (IntendedResolvedState::Staged | IntendedResolvedState::Abandoned, other) => {
                        Err(HolonError::InvalidHolonReference(format!(
                            "ExecutionHolons::lookup for: {:?}, expected STAGED, got: {:?}",
                            intended_resolved_state, other
                        )))
                    }
                    (IntendedResolvedState::Saved, HolonReference::Smart(smart_reference)) => {
                        Ok(HolonReference::Smart(smart_reference.clone()))
                    }
                    (IntendedResolvedState::Saved, other) => {
                        Err(HolonError::InvalidHolonReference(format!(
                            "ExecutionHolons::lookup: expected SAVED, got: {:?}",
                            other
                        )))
                    }
                    (IntendedResolvedState::Transient, _) => {
                        unreachable!("handled on first match arm")
                    }
                    (IntendedResolvedState::Deleted, _) => {
                        unreachable!("handled on first match arm")
                    }
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

    /// Lookup the HolonReference for the previous snapshot (the source token for the prior step).
    pub fn lookup_previous(&self, id: TemporaryId) -> Result<HolonReference, HolonError> {
        let resolved = self.by_temporary_id.get(&id).ok_or_else(|| {
            HolonError::InvalidHolonReference(format!(
                "ExecutionHolons::lookup_previous: no realization recorded for TemporaryId {:?}",
                id,
            ))
        })?;

        resolved.resulting_reference.get_holon_reference()
    }

    // -------------------------------------------------------------------------
    // Introspection helpers
    // -------------------------------------------------------------------------

    /// Lookup the full resolved entry by source token’s `TemporaryId`.
    pub fn get_resolved(&self, temporary_id: &TemporaryId) -> Option<&ResolvedTestReference> {
        self.by_temporary_id.get(temporary_id)
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
