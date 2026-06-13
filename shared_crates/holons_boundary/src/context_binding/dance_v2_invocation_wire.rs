use crate::HolonReferenceWire;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceInvocationReference;
use holons_core::HolonError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// IPC-safe wire wrapper for a new-world `DanceInvocation` holon reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DanceV2InvocationWire {
    pub invocation: HolonReferenceWire,
}

impl DanceV2InvocationWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceInvocationReference, HolonError> {
        DanceInvocationReference::new(self.invocation.bind(context)?)
    }
}

impl From<&DanceInvocationReference> for DanceV2InvocationWire {
    fn from(invocation: &DanceInvocationReference) -> Self {
        Self { invocation: HolonReferenceWire::from(invocation.as_holon_reference()) }
    }
}
