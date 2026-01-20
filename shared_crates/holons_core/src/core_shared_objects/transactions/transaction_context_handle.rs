use std::sync::Arc;

use core_types::HolonError;

use super::{TransactionContext, TxId};

/// Runtime-only transaction carrier for transaction-bound references.
#[derive(Debug, Clone)]
pub struct TransactionContextHandle {
    tx_id: TxId,
    context: Arc<TransactionContext>,
}

impl TransactionContextHandle {
    /// Creates a handle bound to the provided transaction context.
    pub fn new(context: Arc<TransactionContext>) -> Self {
        let tx_id = context.tx_id();
        Self { tx_id, context }
    }

    /// Validates the tx_id against the context before creating the handle.
    pub fn bind(tx_id: TxId, context: Arc<TransactionContext>) -> Result<Self, HolonError> {
        if context.tx_id() != tx_id {
            return Err(HolonError::InvalidParameter(format!(
                "TransactionContextHandle bind failed: wire tx_id {:?} does not match active tx_id {:?}",
                tx_id,
                context.tx_id()
            )));
        }
        Ok(Self { tx_id, context })
    }

    /// Returns the bound transaction id.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Returns a cloned reference to the bound transaction context.
    pub fn context(&self) -> Arc<TransactionContext> {
        Arc::clone(&self.context)
    }
}
