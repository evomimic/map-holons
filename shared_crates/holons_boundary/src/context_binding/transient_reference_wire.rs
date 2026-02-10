use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use holons_core::{HolonError, TransientReference};
use core_types::TemporaryId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientReferenceWire {
    tx_id: TxId,
    id: TemporaryId,
}

impl TransientReferenceWire {
    pub fn new(tx_id: TxId, id: TemporaryId) -> Self {
        Self { tx_id, id }
    }

    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Binds a wire reference to a TransactionContext, validating tx_id and returning a runtime reference.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<TransientReference, HolonError> {
        let context_handle = TransactionContextHandle::bind(self.tx_id(), context)?;
        Ok(TransientReference::from_temporary_id(context_handle, &self.id))
    }
}

impl From<&TransientReference> for TransientReferenceWire {
    fn from(reference: &TransientReference) -> Self {
        Self::new(reference.tx_id(), reference.temporary_id())
    }
}

impl From<TransientReference> for TransientReferenceWire {
    fn from(reference: TransientReference) -> Self {
        TransientReferenceWire::from(&reference)
    }
}
