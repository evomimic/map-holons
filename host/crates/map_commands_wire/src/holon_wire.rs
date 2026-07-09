use std::sync::Arc;

use base_types::BaseValue;
use core_types::{HolonError, PropertyName, RelationshipName};
use holons_boundary::HolonReferenceWire;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use serde::{Deserialize, Serialize};

use map_commands_contract::{HolonAction, HolonCommand, ReadableHolonAction, WritableHolonAction};

/// Holon-scoped wire command.
///
/// Targets a specific holon within an active transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HolonCommandWire {
    pub tx_id: TxId,
    pub target: HolonReferenceWire,
    pub action: HolonActionWire,
}

/// Wire-level holon actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HolonActionWire {
    Read(ReadableHolonActionWire),
    Write(WritableHolonActionWire),
}

/// Wire-level read-only holon actions.
///
/// Mirrors `ReadableHolonAction` — each variant maps to a `ReadableHolon` trait method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReadableHolonActionWire {
    /// `clone_holon()` → `TransientReference`
    CloneHolon,

    /// `summarize()` → `String`
    Summarize,

    /// `holon_id()` → `HolonId`
    GetHolonId,

    /// `predecessor()` → `Option<HolonReference>`
    GetPredecessor,

    /// `key()` → `Option<MapString>`
    GetKey,

    /// `versioned_key()` → `MapString`
    GetVersionedKey,

    /// `property_value(name)` → `Option<PropertyValue>`
    GetPropertyValue { name: PropertyName },

    /// `related_holons(name)` → `HolonCollection`
    GetRelatedHolons { name: RelationshipName },
}

/// Wire-level write (mutating) holon actions.
///
/// Mirrors `WritableHolonAction` — each variant maps to a `WritableHolon` trait method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WritableHolonActionWire {
    /// `with_property_value(name, value)`
    WithPropertyValue { name: PropertyName, value: BaseValue },

    /// `remove_property_value(name)`
    RemovePropertyValue { name: PropertyName },

    /// `add_related_holons(name, holons)`
    AddRelatedHolons { name: RelationshipName, holons: Vec<HolonReferenceWire> },

    /// `remove_related_holons(name, holons)`
    RemoveRelatedHolons { name: RelationshipName, holons: Vec<HolonReferenceWire> },

    /// `with_descriptor(descriptor)`
    WithDescriptor { descriptor: HolonReferenceWire },
}

// ── Binding ─────────────────────────────────────────────────────────

impl HolonCommandWire {
    /// Binds a holon wire command to its domain equivalent.
    ///
    /// Requires a pre-resolved `Arc<TransactionContext>` (looked up from
    /// `RuntimeSession.active_transactions` by the caller).
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<HolonCommand, HolonError> {
        let target = self.target.bind(context)?;
        let action = self.action.bind(context)?;
        Ok(HolonCommand { context: Arc::clone(context), target, action })
    }
}

impl HolonActionWire {
    fn bind(self, context: &Arc<TransactionContext>) -> Result<HolonAction, HolonError> {
        match self {
            HolonActionWire::Read(r) => Ok(HolonAction::Read(r.bind())),
            HolonActionWire::Write(w) => Ok(HolonAction::Write(w.bind(context)?)),
        }
    }
}

impl ReadableHolonActionWire {
    fn bind(self) -> ReadableHolonAction {
        match self {
            ReadableHolonActionWire::CloneHolon => ReadableHolonAction::CloneHolon,
            ReadableHolonActionWire::Summarize => ReadableHolonAction::Summarize,
            ReadableHolonActionWire::GetHolonId => ReadableHolonAction::GetHolonId,
            ReadableHolonActionWire::GetPredecessor => ReadableHolonAction::GetPredecessor,
            ReadableHolonActionWire::GetKey => ReadableHolonAction::GetKey,
            ReadableHolonActionWire::GetVersionedKey => ReadableHolonAction::GetVersionedKey,
            ReadableHolonActionWire::GetPropertyValue { name } => {
                ReadableHolonAction::GetPropertyValue { name }
            }
            ReadableHolonActionWire::GetRelatedHolons { name } => {
                ReadableHolonAction::GetRelatedHolons { name }
            }
        }
    }
}

impl WritableHolonActionWire {
    fn bind(self, context: &Arc<TransactionContext>) -> Result<WritableHolonAction, HolonError> {
        match self {
            WritableHolonActionWire::WithPropertyValue { name, value } => {
                Ok(WritableHolonAction::WithPropertyValue { name, value })
            }
            WritableHolonActionWire::RemovePropertyValue { name } => {
                Ok(WritableHolonAction::RemovePropertyValue { name })
            }
            WritableHolonActionWire::AddRelatedHolons { name, holons } => {
                let mut refs = Vec::with_capacity(holons.len());
                for w in holons {
                    refs.push(w.bind(context)?);
                }
                Ok(WritableHolonAction::AddRelatedHolons { name, holons: refs })
            }
            WritableHolonActionWire::RemoveRelatedHolons { name, holons } => {
                let mut refs = Vec::with_capacity(holons.len());
                for w in holons {
                    refs.push(w.bind(context)?);
                }
                Ok(WritableHolonAction::RemoveRelatedHolons { name, holons: refs })
            }
            WritableHolonActionWire::WithDescriptor { descriptor } => {
                Ok(WritableHolonAction::WithDescriptor { descriptor: descriptor.bind(context)? })
            }
        }
    }
}
