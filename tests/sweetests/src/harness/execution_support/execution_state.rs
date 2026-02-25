//! Minimal execution-time state for running a test case.
//!
//! `TestExecutionState` is the single runtime hub executors use to:
//! - **record** new realizations (e.g., a freshly staged holon), and
//! - **look up** previously realized handles using fixture tokens as snapshot carriers
//!
//! It intentionally **does not** own services; those come from the provided
//! `context` (per Issue #308). This keeps executors focused on the loop:
//! **resolve → execute → validate → record**.
//!
//! Despite the name, this is **not** an enum of lifecycle phases (Running, Done, …).
//! It’s a thin container around [`ExecutionHolons`] so we can grow execution-time
//! concerns later (diagnostics, metrics, history) without changing executor APIs.

use std::sync::Arc;

use crate::harness::{
    execution_support::{ExecutionHolons, ExecutionReference, ResolveBy},
    fixtures_support::TestReference,
};
use holons_prelude::prelude::*;

#[derive(Clone, Debug)]
pub struct TestExecutionState {
    pub context: Arc<TransactionContext>,
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
    pub fn new(test_context: Arc<TransactionContext>) -> Self {
        TestExecutionState { context: test_context, execution_holons: ExecutionHolons::default() }
    }

    /// Reset the state (clears all recorded holons).
    pub fn clear(&mut self) {
        self.execution_holons = ExecutionHolons::new();
    }

    pub fn context(&self) -> Arc<TransactionContext> {
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
    pub fn record(
        &mut self,
        token: &TestReference,
        resolved: ExecutionReference,
    ) -> Result<(), HolonError> {
        self.execution_holons.record(token, resolved)?;

        Ok(())
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
    pub fn resolve_execution_reference(
        &self,
        context: &Arc<TransactionContext>,
        resolution_type: ResolveBy,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        self.execution_holons.resolve_execution_reference(context, resolution_type, token)
    }

    /// Batch variant of `resolve_execution_reference`.
    #[inline]
    pub fn resolve_execution_references(
        &self,
        context: &Arc<TransactionContext>,
        resolution_type: ResolveBy,
        tokens: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        self.execution_holons.resolve_execution_references(context, resolution_type, tokens)
    }
}
