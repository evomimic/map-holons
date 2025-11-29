//! Minimal execution-time state for running a test case.
//!
//! `TestExecutionState` is the single runtime hub executors use to:
//! - **record** new realizations (e.g., a freshly staged holon), and
//! - **look up** previously realized handles from fixture tokens.
//!
//! It intentionally **does not** own services; those come from the provided
//! `context` (per Issue #308). This keeps executors focused on the loop:
//! **lookup → call → record**.
//!
//! Despite the name, this is **not** an enum of lifecycle phases (Running, Done, …).
//! It’s a thin container around [`ExecutionHolons`] so we can grow execution-time
//! concerns later (diagnostics, metrics, history) without changing executor APIs.

use std::sync::Arc;

use crate::harness::execution_support::{ExecutionHolons, ResolvedTestReference};
use crate::harness::fixtures_support::TestReference;
use holons_prelude::prelude::*;

use super::ResultingReference;

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
    /// Overwrites any previous entry for the same `TemporaryId` (most recent wins).
    #[inline]
    pub fn record_resolved(&mut self, resolved: ResolvedTestReference) {
        self.execution_holons.record_resolved(resolved);
    }

    /// Convenience: construct and record from a source token + resulting handle.
    ///
    /// Returns a clone of the `ResolvedTestReference` just stored.
    #[inline]
    pub fn record_from_parts(
        &mut self,
        source_token: TestReference,
        resulting_reference: ResultingReference,
    ) -> ResolvedTestReference {
        self.execution_holons.record_from_parts(source_token, resulting_reference)
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
    pub fn lookup_holon_reference(
        &self,
        context: &dyn HolonsContextBehavior,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        self.execution_holons.lookup_holon_reference(context, token)
    }

    /// Batch variant of `lookup_holon_reference`.
    #[inline]
    pub fn lookup_holon_references(
        &self,
        context: &dyn HolonsContextBehavior,
        tokens: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        self.execution_holons.lookup_holon_references(context, tokens)
    }
}
