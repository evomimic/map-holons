use std::sync::Arc;

use base_types::MapString;
use core_types::{ContentSet, HolonError, HolonId, LocalId};
use holons_boundary::{
    DanceRequestWire, HolonReferenceWire, SmartReferenceWire, TransientReferenceWire,
};
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::query_layer::QueryExpression;
use serde::{Deserialize, Serialize};

use map_commands_contract::{TransactionAction, TransactionCommand};

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
/// Flat enum per the MAP Commands spec. Policy classification is enforced by
/// `CommandDescriptor` at runtime, not by enum structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionActionWire {
    /// Commits the transaction.
    Commit,

    /// Loads holons from uploaded/imported file content.
    LoadHolons { content_set: ContentSet },

    /// Executes a dance request within this transaction.
    Dance(DanceRequestWire),

    /// Executes a query expression within this transaction.
    Query(QueryExpression),

    // ── Lookup actions ───────────────────────────────────────────────
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

    // ── Mutation actions ─────────────────────────────────────────────
    /// `new_holon(key)` → `TransientReference`
    NewHolon { key: Option<MapString> },

    /// `stage_new_holon(source)` → `StagedReference`
    StageNewHolon { source: TransientReferenceWire },

    /// `stage_new_from_clone(original, new_key)` → `StagedReference`
    StageNewFromClone { original: HolonReferenceWire, new_key: MapString },

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
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<TransactionCommand, HolonError> {
        let action = self.action.bind(&context)?;
        Ok(TransactionCommand { context, action })
    }
}

impl TransactionActionWire {
    fn bind(self, context: &Arc<TransactionContext>) -> Result<TransactionAction, HolonError> {
        match self {
            TransactionActionWire::Commit => Ok(TransactionAction::Commit),
            TransactionActionWire::LoadHolons { content_set } => {
                Ok(TransactionAction::LoadHolons { content_set })
            }
            TransactionActionWire::Dance(request_wire) => {
                Ok(TransactionAction::Dance(request_wire.bind(context)?))
            }
            TransactionActionWire::Query(query) => Ok(TransactionAction::Query(query)),

            // Lookup actions — no context binding needed
            TransactionActionWire::GetAllHolons => Ok(TransactionAction::GetAllHolons),
            TransactionActionWire::GetStagedHolonByBaseKey { key } => {
                Ok(TransactionAction::GetStagedHolonByBaseKey { key })
            }
            TransactionActionWire::GetStagedHolonsByBaseKey { key } => {
                Ok(TransactionAction::GetStagedHolonsByBaseKey { key })
            }
            TransactionActionWire::GetStagedHolonByVersionedKey { key } => {
                Ok(TransactionAction::GetStagedHolonByVersionedKey { key })
            }
            TransactionActionWire::GetTransientHolonByBaseKey { key } => {
                Ok(TransactionAction::GetTransientHolonByBaseKey { key })
            }
            TransactionActionWire::GetTransientHolonByVersionedKey { key } => {
                Ok(TransactionAction::GetTransientHolonByVersionedKey { key })
            }
            TransactionActionWire::StagedCount => Ok(TransactionAction::StagedCount),
            TransactionActionWire::TransientCount => Ok(TransactionAction::TransientCount),

            // Mutation actions — some require context binding
            TransactionActionWire::NewHolon { key } => Ok(TransactionAction::NewHolon { key }),
            TransactionActionWire::StageNewHolon { source } => {
                Ok(TransactionAction::StageNewHolon { source: source.bind(context)? })
            }
            TransactionActionWire::StageNewFromClone { original, new_key } => {
                Ok(TransactionAction::StageNewFromClone {
                    original: original.bind(context)?,
                    new_key,
                })
            }
            TransactionActionWire::StageNewVersion { current_version } => {
                Ok(TransactionAction::StageNewVersion {
                    current_version: current_version.bind(context)?,
                })
            }
            TransactionActionWire::StageNewVersionFromId { holon_id } => {
                Ok(TransactionAction::StageNewVersionFromId { holon_id })
            }
            TransactionActionWire::DeleteHolon { local_id } => {
                Ok(TransactionAction::DeleteHolon { local_id })
            }
        }
    }
}
