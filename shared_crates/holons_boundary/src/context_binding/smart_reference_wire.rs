use core_types::{HolonId, PropertyMap};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{HolonError, SmartReference};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartReferenceWire {
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReferenceWire {
    pub fn new(holon_id: HolonId, smart_property_values: Option<PropertyMap>) -> Self {
        Self { holon_id, smart_property_values }
    }

    pub fn holon_id(&self) -> HolonId {
        self.holon_id.clone()
    }

    /// Binds a saved wire reference to the target context's space.
    ///
    /// Saved references are not transaction-bound. The context supplies only the
    /// destination space-read capability; staged and transient wire references
    /// retain their strict transaction validation in their own binders.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<SmartReference, HolonError> {
        let space_read_handle = context.space_read_handle();
        match self.smart_property_values {
            Some(property_values) => Ok(SmartReference::new_with_properties(
                space_read_handle,
                self.holon_id,
                property_values,
            )),
            None => Ok(SmartReference::new_from_id(space_read_handle, self.holon_id)),
        }
    }

    /// Alias retained for uniform `HolonReferenceWire` rebinding. Saved-reference
    /// binding is inherently a bind to the destination space, not a bypass.
    pub fn rebind(self, context: &Arc<TransactionContext>) -> Result<SmartReference, HolonError> {
        self.bind(context)
    }
}

impl From<&SmartReference> for SmartReferenceWire {
    fn from(reference: &SmartReference) -> Self {
        Self::new(reference.holon_id(), reference.smart_property_values().cloned())
    }
}

impl From<SmartReference> for SmartReferenceWire {
    fn from(reference: SmartReference) -> Self {
        SmartReferenceWire::from(&reference)
    }
}
