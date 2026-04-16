use std::sync::Arc;

use base_types::MapString;
use core_types::{ContentSet, HolonId, LocalId};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceRequest;
use holons_core::query_layer::QueryExpression;
use holons_core::reference_layer::{HolonReference, SmartReference, TransientReference};

use super::{CommandDescriptor, MutationClassification};

/// Transaction-scoped domain command.
///
/// The `context` field holds a strong reference to the bound transaction.
/// No TxId strings exist below binding.
#[derive(Debug)]
pub struct TransactionCommand {
    pub context: Arc<TransactionContext>,
    pub action: TransactionAction,
}

/// Domain-level transaction actions.
///
/// Kept flat per the MAP Commands spec. Policy classification (mutating vs read-only)
/// is enforced by `CommandDescriptor` at runtime, not by enum structure.
#[derive(Debug)]
pub enum TransactionAction {
    /// Commits the transaction.
    Commit,

    /// Loads holons from uploaded/imported file content.
    LoadHolons { content_set: ContentSet },

    /// Executes a dance request within this transaction.
    Dance(DanceRequest),

    /// Executes a query expression within this transaction.
    Query(QueryExpression),

    // ── Lookup actions (LookupFacade) ────────────────────────────────
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

    // ── Mutation actions (MutationFacade) ─────────────────────────────
    /// `new_holon(key)` → `TransientReference`
    NewHolon { key: Option<MapString> },

    /// `stage_new_holon(transient_reference)` → `StagedReference`
    StageNewHolon { source: TransientReference },

    /// `stage_new_from_clone(original, new_key)` → `StagedReference`
    StageNewFromClone { original: HolonReference, new_key: MapString },

    /// `stage_new_version(current_version)` → `StagedReference`
    StageNewVersion { current_version: SmartReference },

    /// `stage_new_version_from_id(holon_id)` → `StagedReference`
    StageNewVersionFromId { holon_id: HolonId },

    /// `delete_holon(local_id)` → `()`
    DeleteHolon { local_id: LocalId },
}

impl TransactionAction {
    pub fn descriptor(&self) -> CommandDescriptor {
        match self {
            TransactionAction::Commit => CommandDescriptor::mutating_with_guard(),
            TransactionAction::LoadHolons { .. } => CommandDescriptor::mutating_with_guard(),
            TransactionAction::Dance(_) => CommandDescriptor {
                mutation: MutationClassification::RuntimeDetected,
                requires_open_tx: true,
                requires_commit_guard: false,
            },
            TransactionAction::Query(_) => CommandDescriptor::transaction_read_only(),

            // Lookups
            TransactionAction::GetAllHolons
            | TransactionAction::GetStagedHolonByBaseKey { .. }
            | TransactionAction::GetStagedHolonsByBaseKey { .. }
            | TransactionAction::GetStagedHolonByVersionedKey { .. }
            | TransactionAction::GetTransientHolonByBaseKey { .. }
            | TransactionAction::GetTransientHolonByVersionedKey { .. }
            | TransactionAction::StagedCount
            | TransactionAction::TransientCount => CommandDescriptor::transaction_read_only(),

            // Mutations
            TransactionAction::NewHolon { .. }
            | TransactionAction::StageNewHolon { .. }
            | TransactionAction::StageNewFromClone { .. }
            | TransactionAction::StageNewVersion { .. }
            | TransactionAction::StageNewVersionFromId { .. }
            | TransactionAction::DeleteHolon { .. } => CommandDescriptor::mutating(),
        }
    }
}
