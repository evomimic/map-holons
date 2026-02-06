use crate::core_shared_objects::holon_pool::SerializableHolonPool;

use crate::core_shared_objects::transactions::TxId;
use crate::{HolonReference, HolonReferenceWire};
use serde::{Deserialize, Serialize};

/// `SessionState` represents **transaction-scoped, serializable execution state**
/// that is explicitly transported across IPC boundaries.
///
/// It captures **provisional state owned by the current transaction**—such as
/// staged and transient holons—and enough identifying information to restore
/// that state on the receiving side.
///
/// `SessionState` is intentionally:
/// - **Context-free**: it contains no runtime handles or live references
/// - **Serializable**: suitable for host ↔ guest and UI ↔ host IPC
/// - **Rebindable**: wire-level references are explicitly bound to the active
///   `TransactionContext` at ingress
///
/// This structure is not a general-purpose context object and does not model
/// long-lived space state. It exists solely to support **safe, explicit
/// transfer of transaction-local state** during request/response flows, and
/// may evolve toward token-based or indirect state transfer mechanisms in the future.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SessionState {
    tx_id: Option<TxId>,
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_holon_space: Option<HolonReferenceWire>,
}

impl SessionState {
    /// Creates a new session_state state with the provided staged and transient holons and local holon space.
    pub fn new(
        transient_holons: SerializableHolonPool,
        staged_holons: SerializableHolonPool,
        local_holon_space: Option<HolonReference>,
        tx_id: Option<TxId>,
    ) -> Self {
        Self {
            tx_id,
            transient_holons,
            staged_holons,
            local_holon_space: local_holon_space.map(HolonReferenceWire::from),
        }
    }

    pub fn get_tx_id(&self) -> Option<TxId> {
        self.tx_id
    }

    pub fn set_tx_id(&mut self, tx_id: TxId) {
        self.tx_id = Some(tx_id);
    }

    pub fn get_local_space_holon_wire(&self) -> Option<HolonReferenceWire> {
        self.local_holon_space.clone()
    }

    /// Sets a new local holon space reference.
    pub fn set_local_holon_space(&mut self, local_holon_space: Option<HolonReference>) {
        self.local_holon_space = local_holon_space.map(HolonReferenceWire::from);
    }

    /// Retrieves the staged holon pool.
    pub fn get_staged_holons(&self) -> &SerializableHolonPool {
        &self.staged_holons
    }

    /// Retrieves a mutable reference to the staged holon pool.
    pub fn get_staged_holons_mut(&mut self) -> &mut SerializableHolonPool {
        &mut self.staged_holons
    }

    /// Retrieves the transient holon pool.
    pub fn get_transient_holons(&self) -> &SerializableHolonPool {
        &self.transient_holons
    }

    /// Retrieves a mutable reference to the transient holon pool.
    pub fn get_transient_holons_mut(&mut self) -> &mut SerializableHolonPool {
        &mut self.transient_holons
    }

    /// Sets a new staged holon pool.
    pub fn set_staged_holons(&mut self, staged_holons: SerializableHolonPool) {
        self.staged_holons = staged_holons;
    }

    /// Sets a new transient holon pool.
    pub fn set_transient_holons(&mut self, transient_holons: SerializableHolonPool) {
        self.transient_holons = transient_holons;
    }

    /// Summarizes the session_state state.
    pub fn summarize(&self) -> String {
        format!(
            "\n   tx_id: {:?}, \n   local_holon_space: {:?}, \n  staged holons: {} }}",
            self.tx_id,
            self.local_holon_space,
            self.staged_holons.holons.len(),
        )
    }
}
