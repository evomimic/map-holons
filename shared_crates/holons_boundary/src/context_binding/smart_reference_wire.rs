use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use holons_core::{HolonError, SmartReference};
use core_types::{HolonId, PropertyMap};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartReferenceWire {
    tx_id: TxId,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReferenceWire {
    pub fn new(tx_id: TxId, holon_id: HolonId, smart_property_values: Option<PropertyMap>) -> Self {
        Self { tx_id, holon_id, smart_property_values }
    }

    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    pub fn holon_id(&self) -> HolonId {
        self.holon_id.clone()
    }

    /// Binds a wire reference to a TransactionContext, validating tx_id and returning a SmartReference.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<SmartReference, HolonError> {
        let context_handle = TransactionContextHandle::bind(self.tx_id(), Arc::clone(context))?;
        match self.smart_property_values {
            Some(property_values) => Ok(SmartReference::new_with_properties(
                context_handle,
                self.holon_id,
                property_values,
            )),
            None => Ok(SmartReference::new_from_id(context_handle, self.holon_id)),
        }
    }
}

impl From<&SmartReference> for SmartReferenceWire {
    fn from(reference: &SmartReference) -> Self {
        Self::new(
            reference.tx_id(),
            reference.holon_id(),
            reference.smart_property_values().cloned(),
        )
    }
}

impl From<SmartReference> for SmartReferenceWire {
    fn from(reference: SmartReference) -> Self {
        SmartReferenceWire::from(&reference)
    }
}
