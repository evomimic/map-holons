//! Minimal execution-time state for running a test case.
//!
//! `TestExecutionState` is the single runtime hub executors use to:
//! - **record** new realizations (e.g., a freshly staged holon), and
//! - **look up** previously realized handles from fixture tokens.
//!
//! It intentionally **does not** own services; those come from the provided
//! `context` (per Issue #308). This keeps executors focused on the loop:
//! **resolve → execute → validate → record**.
//!
//! Despite the name, this is **not** an enum of lifecycle phases (Running, Done, …).
//! It’s a thin container around [`ExecutionHolons`] so we can grow execution-time
//! concerns later (diagnostics, metrics, history) without changing executor APIs.

use std::sync::Arc;

use crate::SnapshotId;
use crate::harness::execution_support::{ExecutionHolons, ExecutionReference};
use crate::harness::fixtures_support::TestReference;
use holons_prelude::prelude::*;

#[derive(Clone, Debug)]
pub struct TestExecutionState {
    pub context: Arc<dyn HolonsContextBehavior>,
    // Registry of realized references keyed by source token’s `TemporaryId`.
    pub execution_holons: ExecutionHolons,
}

impl TestExecutionState {
    /// Creates a new `DanceTestExecutionState`.
    ///
    /// # Arguments
    /// - `test_context`: The test execution context.
    /// - `dance_call_service`: The `DanceCallService` instance for managing dance calls.
    ///
    /// # Returns
    /// A new `DanceTestExecutionState` instance.
    pub fn new(test_context: Arc<dyn HolonsContextBehavior>) -> Self {
        TestExecutionState { context: test_context, execution_holons: ExecutionHolons::default() }
    }

    /// Reset the state (clears all recorded holons).
    pub fn clear(&mut self) {
        self.execution_holons = ExecutionHolons::new();
    }

    pub fn context(&self) -> Arc<dyn HolonsContextBehavior> {
        self.context.clone()
    }

    /// Borrow the registry (read-only).
    #[inline]
    pub fn holons(&self) -> &ExecutionHolons {
        &self.execution_holons
    }

    /// Borrow the registry (mutable).
    #[inline]
    pub fn holons_mut(&mut self) -> &mut ExecutionHolons {
        &mut self.execution_holons
    }

    // ---------------------------------------------------------------------
    // Recording (Realization → Registry)
    // ---------------------------------------------------------------------

    /// Record a fully-built resolved entry produced by this step.
    ///
    /// Append-only, cannot overwrite existing.
    #[inline]
    pub fn record(&mut self, id: SnapshotId, resolved: ExecutionReference) {
        self.execution_holons.record(id, resolved);
    }

    // ---------------------------------------------------------------------
    // Lookup (Tokens → HolonReference) for executor inputs
    // ---------------------------------------------------------------------

    /// Convert a fixture token into the **current** runtime handle for use as input.
    ///
    /// Enforces the invariant: **Saved ≙ Staged(Committed(LocalId))**.
    /// No Nursery/DHT fallback is performed; missing entries are treated as
    /// authoring/ordering errors.
    #[inline]
    pub fn resolve_source_reference(
        &self,
        context: &dyn HolonsContextBehavior,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        self.execution_holons.resolve_source_reference(context, token)
    }

    /// Batch variant of `resolve_source_reference`.
    #[inline]
    pub fn resolve_source_references(
        &self,
        context: &dyn HolonsContextBehavior,
        tokens: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        self.execution_holons.resolve_source_references(context, tokens)
    }

}
