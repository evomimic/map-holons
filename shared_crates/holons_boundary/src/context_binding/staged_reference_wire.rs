use core_types::TemporaryId;
use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use holons_core::{HolonError, StagedReference};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedReferenceWire {
    tx_id: TxId,
    id: TemporaryId,
}

impl StagedReferenceWire {
    pub fn new(tx_id: TxId, id: TemporaryId) -> Self {
        Self { tx_id, id }
    }

    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Binds a wire reference to a TransactionContext, validating tx_id and returning a runtime reference.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<StagedReference, HolonError> {
        let context_handle = TransactionContextHandle::bind(self.tx_id(), context)?;
        Ok(StagedReference::from_temporary_id(context_handle, &self.id))
    }

    /// Rebinds this wire reference to a different transaction context, bypassing
    /// the tx_id validation that [`bind`](Self::bind) performs.
    ///
    /// Unlike `bind`, which enforces that the wire's embedded tx_id matches the
    /// target context (catching accidental use of stale references), `rebind`
    /// intentionally discards the original tx_id and preserves only the
    /// TemporaryId. The caller is responsible for ensuring that the target
    /// context's nursery contains a staged holon with this TemporaryId.
    ///
    /// Primary use case: re-importing serialized fixture or session data into a
    /// newly opened transaction.
    pub fn rebind(self, context: &Arc<TransactionContext>) -> Result<StagedReference, HolonError> {
        let context_handle = TransactionContextHandle::new(Arc::clone(context));
        Ok(StagedReference::from_temporary_id(context_handle, &self.id))
    }
}

impl From<&StagedReference> for StagedReferenceWire {
    fn from(reference: &StagedReference) -> Self {
        Self::new(reference.tx_id(), reference.temporary_id())
    }
}

impl From<StagedReference> for StagedReferenceWire {
    fn from(reference: StagedReference) -> Self {
        StagedReferenceWire::from(&reference)
    }
}
