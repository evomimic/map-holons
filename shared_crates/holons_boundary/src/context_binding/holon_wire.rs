use crate::context_binding::staged_wire::StagedHolonWire;
use crate::context_binding::transient_wire::TransientHolonWire;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::{Holon, SavedHolon};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HolonWire {
    Transient(TransientHolonWire),
    Staged(StagedHolonWire),
    Saved(SavedHolon),
}

impl HolonWire {
    /// Binds this holon's nested references to a TransactionContext, validating tx_id.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<Holon, HolonError> {
        Ok(match self {
            HolonWire::Transient(holon) => Holon::Transient(holon.bind(context)?),
            HolonWire::Staged(holon) => Holon::Staged(holon.bind(context)?),
            HolonWire::Saved(holon) => Holon::Saved(holon),
        })
    }

    /// Rebinds this holon's nested references to a different transaction
    /// context, bypassing tx_id validation. Delegates to the variant's `rebind`.
    pub fn rebind(self, context: &Arc<TransactionContext>) -> Result<Holon, HolonError> {
        Ok(match self {
            HolonWire::Transient(holon) => Holon::Transient(holon.rebind(context)?),
            HolonWire::Staged(holon) => Holon::Staged(holon.rebind(context)?),
            HolonWire::Saved(holon) => Holon::Saved(holon),
        })
    }
}

impl From<&Holon> for HolonWire {
    fn from(value: &Holon) -> Self {
        match value {
            Holon::Transient(holon) => HolonWire::Transient(TransientHolonWire::from(holon)),
            Holon::Staged(holon) => HolonWire::Staged(StagedHolonWire::from(holon)),
            Holon::Saved(holon) => HolonWire::Saved(holon.clone()),
        }
    }
}
