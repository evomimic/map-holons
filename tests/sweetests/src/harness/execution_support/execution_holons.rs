//! Runtime registry and lookup utilities for realized references.
//!
//! `ExecutionHolons` maps a fixture token’s `TemporaryId` to the runtime
//! handle produced by a step (e.g., a newly staged holon). Executors
//! record new realizations here, and later steps *look them up* to obtain
//! the handles they need to call dances.
//!
//! ## Key points
//! - **Record**: after a step realizes something, wrap it in a
//!   `ExecutionReference` and store it under the expected_snapshot's `TemporaryId`.
//! - **Lookup (inputs)**: executors use `resolve_execution_reference(_vec)` to turn
//!   tokens into live `HolonReference`s *without* touching the Nursery or DHT.
//! - **Invariant**: Saved ≙ Staged(Committed(LocalId)) is enforced during lookup.
//! - **Append-only**: Overwrites are not allowed.

use crate::harness::{
    execution_support::ExecutionReference,
    fixtures_support::{SnapshotId, TestHolonState, TestReference},
};
use core_types::{LocalId, TemporaryId};
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Authoritative execution-time registry.
///
/// - Append-only: once a SnapshotId is recorded, it must not be overwritten.
/// - Many SnapshotIds may map to the same ExecutionReference.
/// - Required for correct downstream resolution.
#[derive(Clone, Debug, Default)]
pub struct ExecutionHolons {
    pub by_snapshot_id: BTreeMap<SnapshotId, ExecutionReference>,
}

impl ExecutionHolons {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // -------------------------------------------------------------------------
    // Recording (Realization → Registry)
    // -------------------------------------------------------------------------

    /// Record the execution result for the given token.
    ///
    /// Rules:
    /// - Must be called exactly once per executed step.
    /// - Must record against the step’s Expected SnapshotId (never SourceSnapshot).
    /// - Must not overwrite existing entries.
    pub fn record(
        &mut self,
        token: &TestReference,
        resolved: ExecutionReference,
    ) -> Result<(), HolonError> {
        let id = token.expected_id();
        if self.by_snapshot_id.contains_key(&id) {
            return Err(HolonError::InvalidParameter(format!(
                "An ExecutionHolon already exists for id: {:?}, cannot overwrite.",
                id
            )));
        } else {
            self.by_snapshot_id.insert(id, resolved);
        }

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Lookup (Tokens → HolonReference)
    // -------------------------------------------------------------------------

    /// Turn a fixture token into the **current** runtime `HolonReference` to use as executor input.
    ///
    /// Lookup strategy:
    /// - `TestHolonState::Transient`  → must find a recorded `TransientReference`.
    /// - `TestHolonState::Staged`     → must find a recorded `StagedReference` **not committed**.
    /// - `TestHolonState::Saved`      → must find a recorded `StagedReference` **committed**.
    /// - `TestHolonState::Abandoned`  → must find a recorded `StagedReference` **abandoned**.
    /// - `TestHolonState::Deleted`  → return Error
    pub fn resolve_execution_reference(
        &self,
        context: &Arc<TransactionContext>,
        resolution_type: ResolveBy,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        let SnapshotDeconstruction { id, state } =
            SnapshotDeconstruction::new(resolution_type, token);
        match state {
            TestHolonState::Deleted => Err(HolonError::InvalidParameter(
                "Holon marked as deleted, there is no associated resolved HolonReference"
                    .to_string(),
            )),
            TestHolonState::Transient => {
                let resolved = self
                    .by_snapshot_id
                    .get(&id)
                    .ok_or_else(|| HolonError::InvalidHolonReference(format!(
                        "ExecutionHolons::lookup: no realization recorded for TemporaryId {:?} (expected {:?})",
                        id,
                        state
                    )))?;
                let holon_reference = &resolved.execution_handle.get_holon_reference()?;
                if !holon_reference.is_transient() {
                    return Err(HolonError::InvalidHolonReference(format!(
                        "ExecutionHolons::lookup expected TRANSIENT but got {:?} ",
                        holon_reference
                    )));
                }

                Ok(holon_reference.clone())
            }
            state => {
                let resolved = self
                    .by_snapshot_id
                    .get(&id)
                    .ok_or_else(|| HolonError::InvalidHolonReference(format!(
                        "ExecutionHolons::lookup: no realization recorded for TemporaryId {:?} (expected {:?})",
                        id,
                        state
                    )))?;
                let holon_reference = &resolved.execution_handle.get_holon_reference()?;
                match (state, holon_reference) {
                    (
                        TestHolonState::Staged | TestHolonState::Abandoned,
                        HolonReference::Staged(staged_reference),
                    ) => {
                        if !staged_reference
                            .is_in_state(context, StagedState::Committed(LocalId(Vec::new())))?
                        {
                            Ok(HolonReference::Staged(staged_reference.clone()))
                        } else {
                            Err(HolonError::InvalidHolonReference(
                                    format!("ExecutionHolons::lookup for: {:?}, expected STAGED (not committed) got StagedReference but in StagedState::Committed", state),
                                ))
                        }
                    }
                    (TestHolonState::Staged | TestHolonState::Abandoned, other) => {
                        Err(HolonError::InvalidHolonReference(format!(
                            "ExecutionHolons::lookup for: {:?}, expected STAGED, got: {:?}",
                            state, other
                        )))
                    }
                    (TestHolonState::Saved, HolonReference::Smart(smart_reference)) => {
                        Ok(HolonReference::Smart(smart_reference.clone()))
                    }
                    (TestHolonState::Saved, other) => Err(HolonError::InvalidHolonReference(
                        format!("ExecutionHolons::lookup: expected SAVED, got: {:?}", other),
                    )),
                    (TestHolonState::Transient, _) => {
                        unreachable!("handled on first match arm")
                    }
                    (TestHolonState::Deleted, _) => {
                        unreachable!("handled on first match arm")
                    }
                }
            }
        }
    }

    /// Batch variant of `resolve_execution_reference`.
    pub fn resolve_execution_references(
        &self,
        context: &Arc<TransactionContext>,
        resolution_type: ResolveBy,
        tokens: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        let mut references = Vec::new();
        for token in tokens {
            let reference =
                self.resolve_execution_reference(context, resolution_type.clone(), token)?;
            references.push(reference);
        }
        Ok(references)
    }
}

// -- HELPERS -- //

/// Helper type to determine which snapshot to resovle.
#[derive(Clone)]
pub enum ResolveBy {
    Expected,
    Source,
}

/// Helper type for desconstructing inner fields of a Snapshot.
pub struct SnapshotDeconstruction {
    pub id: TemporaryId,
    pub state: TestHolonState,
}
impl SnapshotDeconstruction {
    pub fn new(resolution_type: ResolveBy, token: &TestReference) -> Self {
        match resolution_type {
            ResolveBy::Source => {
                let snapshot = token.source_snapshot();
                let id = snapshot.id();
                let state = snapshot.state();
                Self { id, state }
            }
            ResolveBy::Expected => {
                let snapshot = token.expected_snapshot();
                let id = snapshot.id();
                let state = snapshot.state();
                Self { id, state }
            }
        }
    }
}
