//! Minimal execution-time state for running a test case.
//!
//! `TestExecutionState` is the single runtime hub executors use to:
//! - **dispatch** commands through `Runtime::execute_command()`
//! - **record** new realizations (e.g., a freshly staged holon), and
//! - **look up** previously realized handles using fixture tokens as snapshot carriers
//!
//! It owns a `Runtime` and tracks the currently active transaction via `active_tx_id`.
//! The `context()` accessor resolves the active transaction on demand from the session.
//!
//! Despite the name, this is **not** an enum of lifecycle phases (Running, Done, …).
//! It's a thin container around [`ExecutionHolons`] so we can grow execution-time
//! concerns later (diagnostics, metrics, history) without changing executor APIs.

use std::sync::Arc;

use crate::harness::{
    execution_support::{ExecutionHolons, ExecutionReference, ResolveBy},
    fixtures_support::TestReference,
};
use holons_boundary::SerializableHolonPool;
use holons_prelude::prelude::*;

use holons_core::core_shared_objects::transactions::TxId;
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use map_commands_runtime::Runtime;
use tracing::debug;

#[derive(Clone, Debug)]
pub struct TestExecutionState {
    runtime: Runtime,
    active_tx_id: TxId,
    fixture_transient_holons: SerializableHolonPool,
    // Registry of realized references keyed by source token's `TemporaryId`.
    execution_holons: ExecutionHolons,
}

impl TestExecutionState {
    pub fn new(
        runtime: Runtime,
        tx_id: TxId,
        fixture_transient_holons: SerializableHolonPool,
    ) -> Self {
        TestExecutionState {
            runtime,
            active_tx_id: tx_id,
            fixture_transient_holons,
            execution_holons: ExecutionHolons::default(),
        }
    }

    /// Reset the state (clears all recorded holons).
    pub fn clear(&mut self) {
        self.execution_holons = ExecutionHolons::new();
    }

    /// Returns the active transaction context, resolved on demand from the session.
    pub fn context(&self) -> Arc<TransactionContext> {
        self.runtime
            .session()
            .get_transaction(&self.active_tx_id)
            .expect("active transaction must exist in session")
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn active_tx_id(&self) -> TxId {
        self.active_tx_id
    }

    pub fn set_active_tx_id(&mut self, tx_id: TxId) {
        self.active_tx_id = tx_id;
    }

    /// Makes a newly opened transaction active and imports fixture transients into it.
    pub fn activate_transaction(&mut self, tx_id: TxId) -> Result<(), HolonError> {
        let context = self.runtime.session().get_transaction(&tx_id)?;
        self.import_fixture_transient_holons(&context)?;
        self.active_tx_id = tx_id;
        Ok(())
    }

    fn import_fixture_transient_holons(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        if self.fixture_transient_holons.holons.is_empty() {
            return Ok(());
        }

        let bound_transient_holons = self.fixture_transient_holons.clone().rebind(context)?;
        context.import_transient_holons(bound_transient_holons)
    }

    /// Returns an open transaction context for assertion-style helper steps.
    /// This allows for db inspection after commit, bypassing transaction lifecycle checks.
    ///
    /// If the active transaction is still open, reuse it. If it has already
    /// been committed, open a fresh observer transaction without changing the
    /// harness's active transaction lifecycle.
    pub async fn open_assertion_context(
        &self,
        step_name: &str,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        let context = self.context();
        if context.is_open() {
            return Ok(context);
        }

        let result = self
            .dispatch_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                &format!("{step_name}: begin_assertion_transaction"),
            )
            .await?;

        match result {
            MapResult::TransactionCreated { tx_id } => {
                let context = self.runtime.session().get_transaction(&tx_id)?;
                self.import_fixture_transient_holons(&context)?;
                Ok(context)
            }
            other => Err(HolonError::InvalidParameter(format!(
                "{step_name}: expected TransactionCreated, got {:?}",
                other
            ))),
        }
    }

    /// Dispatch a MapCommand through the runtime.
    ///
    /// Thin pass-through with logging. Executors own their own result validation.
    pub async fn dispatch_command(
        &self,
        command: MapCommand,
        step_name: &str,
    ) -> Result<MapResult, HolonError> {
        debug!("Dispatching {}: {:?}", step_name, command);
        let result = self.runtime.execute_command(command).await;
        debug!("{} result: {:?}", step_name, &result);
        result
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
