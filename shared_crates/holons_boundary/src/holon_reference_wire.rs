use crate::smart_reference_wire::SmartReferenceWire;
use crate::staged_reference_wire::StagedReferenceWire;
use crate::transient_reference_wire::TransientReferenceWire;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::HolonError;
use holons_core::HolonReference;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HolonReferenceWire {
    Transient(TransientReferenceWire),
    Staged(StagedReferenceWire),
    Smart(SmartReferenceWire),
}

impl HolonReferenceWire {
    /// Binds a wire reference enum to a TransactionContext, validating tx_id.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<HolonReference, HolonError> {
        match self {
            HolonReferenceWire::Transient(transient) => {
                transient.bind(context).map(HolonReference::Transient)
            }
            HolonReferenceWire::Staged(staged) => staged.bind(context).map(HolonReference::Staged),
            HolonReferenceWire::Smart(smart) => smart.bind(context).map(HolonReference::Smart),
        }
    }
}

impl From<HolonReference> for HolonReferenceWire {
    fn from(reference: HolonReference) -> Self {
        match reference {
            HolonReference::Transient(transient) => {
                HolonReferenceWire::Transient(TransientReferenceWire::from(transient))
            }
            HolonReference::Staged(staged) => {
                HolonReferenceWire::Staged(StagedReferenceWire::from(staged))
            }
            HolonReference::Smart(smart) => {
                HolonReferenceWire::Smart(SmartReferenceWire::from(smart))
            }
        }
    }
}

impl From<&HolonReference> for HolonReferenceWire {
    fn from(reference: &HolonReference) -> Self {
        match reference {
            HolonReference::Transient(transient) => {
                HolonReferenceWire::Transient(TransientReferenceWire::from(transient))
            }
            HolonReference::Staged(staged) => {
                HolonReferenceWire::Staged(StagedReferenceWire::from(staged))
            }
            HolonReference::Smart(smart) => {
                HolonReferenceWire::Smart(SmartReferenceWire::from(smart))
            }
        }
    }
}
