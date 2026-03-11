use std::sync::Arc;

use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId};
use holons_boundary::{
    DanceRequestWire, HolonReferenceWire, SmartReferenceWire, TransientReferenceWire,
};
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::query_layer::QueryExpression;
use serde::{Deserialize, Serialize};

use crate::domain::{
    LookupAction, MutationAction, TransactionAction, TransactionCommand,
};

/// Transaction-scoped wire command.
///
/// Carries a TxId for binding to a live transaction context at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionCommandWire {
    pub tx_id: TxId,
    pub action: TransactionActionWire,
}

/// Wire-level transaction actions.
///
/// All spec variants are defined upfront. Unimplemented variants return
/// `NotImplemented` at dispatch time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionActionWire {
    /// Commits the transaction.
    Commit,

    /// Loads holons from a content bundle.
    LoadHolons { bundle: HolonReferenceWire },

    /// Executes a dance request within this transaction.
    Dance(DanceRequestWire),

    /// Executes a lookup action within this transaction.
    Lookup(LookupActionWire),

    /// Executes a mutation action within this transaction.
    Mutation(MutationActionWire),

    /// Executes a query expression within this transaction.
    Query(QueryExpression),
}

/// Wire-level lookup actions.
///
/// Mirrors `LookupAction` — each variant maps to a `LookupFacade` method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LookupActionWire {
    /// `get_all_holons()` → `HolonCollection`
    GetAllHolons,

    /// `get_staged_holon_by_base_key(key)` → `StagedReference`
    GetStagedHolonByBaseKey { key: MapString },

    /// `get_staged_holons_by_base_key(key)` → `Vec<StagedReference>`
    GetStagedHolonsByBaseKey { key: MapString },

    /// `get_staged_holon_by_versioned_key(key)` → `StagedReference`
    GetStagedHolonByVersionedKey { key: MapString },

    /// `get_transient_holon_by_base_key(key)` → `TransientReference`
    GetTransientHolonByBaseKey { key: MapString },

    /// `get_transient_holon_by_versioned_key(key)` → `TransientReference`
    GetTransientHolonByVersionedKey { key: MapString },

    /// `staged_count()` → `i64`
    StagedCount,

    /// `transient_count()` → `i64`
    TransientCount,
}

/// Wire-level mutation actions.
///
/// Mirrors `MutationAction` — each variant maps to a `MutationFacade` method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutationActionWire {
    /// `new_holon(key)` → `TransientReference`
    NewHolon { key: Option<MapString> },

    /// `stage_new_holon(source)` → `StagedReference`
    StageNewHolon { source: TransientReferenceWire },

    /// `stage_new_from_clone(original, new_key)` → `StagedReference`
    StageNewFromClone {
        original: HolonReferenceWire,
        new_key: MapString,
    },

    /// `stage_new_version(current_version)` → `StagedReference`
    StageNewVersion { current_version: SmartReferenceWire },

    /// `stage_new_version_from_id(holon_id)` → `StagedReference`
    StageNewVersionFromId { holon_id: HolonId },

    /// `delete_holon(local_id)` → `()`
    DeleteHolon { local_id: LocalId },
}

// ── Binding ─────────────────────────────────────────────────────────

impl TransactionCommandWire {
    /// Binds a transaction wire command to its domain equivalent.
    ///
    /// Requires a pre-resolved `Arc<TransactionContext>` (looked up from
    /// `RuntimeSession.active_transactions` by the caller).
    pub fn bind(
        self,
        context: Arc<TransactionContext>,
    ) -> Result<TransactionCommand, HolonError> {
        let action = self.action.bind(&context)?;
        Ok(TransactionCommand { context, action })
    }
}

impl TransactionActionWire {
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<TransactionAction, HolonError> {
        match self {
            TransactionActionWire::Commit => Ok(TransactionAction::Commit),
            TransactionActionWire::LoadHolons { bundle } => {
                Ok(TransactionAction::LoadHolons {
                    bundle: bundle.bind(context)?,
                })
            }
            TransactionActionWire::Dance(request_wire) => {
                Ok(TransactionAction::Dance(request_wire.bind(context)?))
            }
            TransactionActionWire::Lookup(lookup_wire) => {
                Ok(TransactionAction::Lookup(lookup_wire.bind()))
            }
            TransactionActionWire::Mutation(mutation_wire) => {
                Ok(TransactionAction::Mutation(mutation_wire.bind(context)?))
            }
            TransactionActionWire::Query(query) => Ok(TransactionAction::Query(query)),
        }
    }
}

impl LookupActionWire {
    fn bind(self) -> LookupAction {
        match self {
            LookupActionWire::GetAllHolons => LookupAction::GetAllHolons,
            LookupActionWire::GetStagedHolonByBaseKey { key } => {
                LookupAction::GetStagedHolonByBaseKey { key }
            }
            LookupActionWire::GetStagedHolonsByBaseKey { key } => {
                LookupAction::GetStagedHolonsByBaseKey { key }
            }
            LookupActionWire::GetStagedHolonByVersionedKey { key } => {
                LookupAction::GetStagedHolonByVersionedKey { key }
            }
            LookupActionWire::GetTransientHolonByBaseKey { key } => {
                LookupAction::GetTransientHolonByBaseKey { key }
            }
            LookupActionWire::GetTransientHolonByVersionedKey { key } => {
                LookupAction::GetTransientHolonByVersionedKey { key }
            }
            LookupActionWire::StagedCount => LookupAction::StagedCount,
            LookupActionWire::TransientCount => LookupAction::TransientCount,
        }
    }
}

impl MutationActionWire {
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<MutationAction, HolonError> {
        match self {
            MutationActionWire::NewHolon { key } => Ok(MutationAction::NewHolon { key }),
            MutationActionWire::StageNewHolon { source } => {
                Ok(MutationAction::StageNewHolon {
                    source: source.bind(context)?,
                })
            }
            MutationActionWire::StageNewFromClone { original, new_key } => {
                Ok(MutationAction::StageNewFromClone {
                    original: original.bind(context)?,
                    new_key,
                })
            }
            MutationActionWire::StageNewVersion { current_version } => {
                Ok(MutationAction::StageNewVersion {
                    current_version: current_version.bind(context)?,
                })
            }
            MutationActionWire::StageNewVersionFromId { holon_id } => {
                Ok(MutationAction::StageNewVersionFromId { holon_id })
            }
            MutationActionWire::DeleteHolon { local_id } => {
                Ok(MutationAction::DeleteHolon { local_id })
            }
        }
    }
}
