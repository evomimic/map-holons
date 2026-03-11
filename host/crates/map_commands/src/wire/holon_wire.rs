use std::sync::Arc;

use base_types::BaseValue;
use core_types::{HolonError, PropertyName, RelationshipName};
use holons_boundary::HolonReferenceWire;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use serde::{Deserialize, Serialize};

use crate::domain::{
    HolonAction, HolonCommand, ReadableHolonAction, WritableHolonAction,
};

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

    /// `essential_content()` → `EssentialHolonContent`
    EssentialContent,

    /// `summarize()` → `String`
    Summarize,

    /// `holon_id()` → `HolonId`
    HolonId,

    /// `predecessor()` → `Option<HolonReference>`
    Predecessor,

    /// `key()` → `Option<MapString>`
    Key,

    /// `versioned_key()` → `MapString`
    VersionedKey,

    /// `all_related_holons()` → `RelationshipMap`
    AllRelatedHolons,

    /// `property_value(name)` → `Option<PropertyValue>`
    PropertyValue { name: PropertyName },

    /// `related_holons(name)` → `HolonCollection`
    RelatedHolons { name: RelationshipName },
}

/// Wire-level write (mutating) holon actions.
///
/// Mirrors `WritableHolonAction` — each variant maps to a `WritableHolon` trait method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WritableHolonActionWire {
    /// `with_property_value(name, value)`
    WithPropertyValue {
        name: PropertyName,
        value: BaseValue,
    },

    /// `remove_property_value(name)`
    RemovePropertyValue { name: PropertyName },

    /// `add_related_holons(name, holons)`
    AddRelatedHolons {
        name: RelationshipName,
        holons: Vec<HolonReferenceWire>,
    },

    /// `remove_related_holons(name, holons)`
    RemoveRelatedHolons {
        name: RelationshipName,
        holons: Vec<HolonReferenceWire>,
    },

    /// `with_descriptor(descriptor)`
    WithDescriptor { descriptor: HolonReferenceWire },

    /// `with_predecessor(predecessor)`
    WithPredecessor {
        predecessor: Option<HolonReferenceWire>,
    },
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
        Ok(HolonCommand { target, action })
    }
}

impl HolonActionWire {
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonAction, HolonError> {
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
            ReadableHolonActionWire::EssentialContent => ReadableHolonAction::EssentialContent,
            ReadableHolonActionWire::Summarize => ReadableHolonAction::Summarize,
            ReadableHolonActionWire::HolonId => ReadableHolonAction::HolonId,
            ReadableHolonActionWire::Predecessor => ReadableHolonAction::Predecessor,
            ReadableHolonActionWire::Key => ReadableHolonAction::Key,
            ReadableHolonActionWire::VersionedKey => ReadableHolonAction::VersionedKey,
            ReadableHolonActionWire::AllRelatedHolons => ReadableHolonAction::AllRelatedHolons,
            ReadableHolonActionWire::PropertyValue { name } => {
                ReadableHolonAction::PropertyValue { name }
            }
            ReadableHolonActionWire::RelatedHolons { name } => {
                ReadableHolonAction::RelatedHolons { name }
            }
        }
    }
}

impl WritableHolonActionWire {
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<WritableHolonAction, HolonError> {
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
                Ok(WritableHolonAction::AddRelatedHolons {
                    name,
                    holons: refs,
                })
            }
            WritableHolonActionWire::RemoveRelatedHolons { name, holons } => {
                let mut refs = Vec::with_capacity(holons.len());
                for w in holons {
                    refs.push(w.bind(context)?);
                }
                Ok(WritableHolonAction::RemoveRelatedHolons {
                    name,
                    holons: refs,
                })
            }
            WritableHolonActionWire::WithDescriptor { descriptor } => {
                Ok(WritableHolonAction::WithDescriptor {
                    descriptor: descriptor.bind(context)?,
                })
            }
            WritableHolonActionWire::WithPredecessor { predecessor } => {
                let bound = predecessor.map(|p| p.bind(context)).transpose()?;
                Ok(WritableHolonAction::WithPredecessor {
                    predecessor: bound,
                })
            }
        }
    }
}
