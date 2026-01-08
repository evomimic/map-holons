//! Minimal transaction behavior surface.

use std::fmt::Debug;

use super::TxId;

/// Minimal transaction behavior interface for transaction-scoped contexts.
pub trait TransactionBehavior: Debug + Send + Sync {
    /// Ephemeral transaction identifier.
    fn tx_id(&self) -> TxId;

    /// Whether this transaction is still open.
    fn is_open(&self) -> bool;
}
