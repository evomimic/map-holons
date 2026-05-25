use std::sync::Arc;

use base_types::MapString;
use core_types::{ContentSet, HolonId, LocalId};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceRequest;
use holons_core::reference_layer::{HolonReference, SmartReference, TransientReference};

use super::{CommandLifecyclePolicy, MutationClassification};

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
/// is enforced by `CommandLifecyclePolicy` at runtime, not by enum structure.
#[derive(Debug)]
pub enum TransactionAction {
    /// Commits the transaction.
    Commit,

    /// Undoes the last mutation in this transaction.
    UndoLast,

    /// Redoes the last undone mutation in this transaction.
    RedoLast,

    /// Undoes mutations up to the specified marker.
    UndoToMarker { marker_id: String },

    /// Redoes mutations up to the specified marker.
    RedoToMarker { marker_id: String },

    /// Loads holons from uploaded/imported file content.
    LoadHolons { content_set: ContentSet },

    /// Executes a dance request within this transaction.
    Dance(DanceRequest),

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
    GetStagedCount,

    /// `transient_count()` → `i64`
    GetTransientCount,

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
    pub fn policy(&self) -> CommandLifecyclePolicy {
        match self {
            TransactionAction::Commit => CommandLifecyclePolicy::mutating_with_guard(),
            TransactionAction::UndoLast | TransactionAction::RedoLast => {
                CommandLifecyclePolicy::transaction_read_only()
            }
            TransactionAction::UndoToMarker { .. } | TransactionAction::RedoToMarker { .. } => {
                CommandLifecyclePolicy::transaction_read_only()
            }
            TransactionAction::LoadHolons { .. } => CommandLifecyclePolicy::mutating_with_guard(),
            TransactionAction::Dance(_) => CommandLifecyclePolicy {
                mutation: MutationClassification::RuntimeDetected,
                requires_open_tx: true,
                requires_commit_guard: false,
            },
            // Lookups
            TransactionAction::GetAllHolons
            | TransactionAction::GetStagedHolonByBaseKey { .. }
            | TransactionAction::GetStagedHolonsByBaseKey { .. }
            | TransactionAction::GetStagedHolonByVersionedKey { .. }
            | TransactionAction::GetTransientHolonByBaseKey { .. }
            | TransactionAction::GetTransientHolonByVersionedKey { .. }
            | TransactionAction::GetStagedCount
            | TransactionAction::GetTransientCount => {
                CommandLifecyclePolicy::transaction_read_only()
            }

            // Mutations
            TransactionAction::NewHolon { .. }
            | TransactionAction::StageNewHolon { .. }
            | TransactionAction::StageNewFromClone { .. }
            | TransactionAction::StageNewVersion { .. }
            | TransactionAction::StageNewVersionFromId { .. }
            | TransactionAction::DeleteHolon { .. } => CommandLifecyclePolicy::mutating(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TransactionAction::Commit => "commit",
            TransactionAction::UndoLast => "undo_last",
            TransactionAction::RedoLast => "redo_last",
            TransactionAction::UndoToMarker { .. } => "undo_to_marker",
            TransactionAction::RedoToMarker { .. } => "redo_to_marker",
            TransactionAction::LoadHolons { .. } => "load_holons",
            TransactionAction::Dance(_) => "dance",
            TransactionAction::GetAllHolons => "get_all_holons",
            TransactionAction::GetStagedHolonByBaseKey { .. } => "get_staged_holon_by_base_key",
            TransactionAction::GetStagedHolonsByBaseKey { .. } => "get_staged_holons_by_base_key",
            TransactionAction::GetStagedHolonByVersionedKey { .. } => {
                "get_staged_holon_by_versioned_key"
            }
            TransactionAction::GetTransientHolonByBaseKey { .. } => {
                "get_transient_holon_by_base_key"
            }
            TransactionAction::GetTransientHolonByVersionedKey { .. } => {
                "get_transient_holon_by_versioned_key"
            }
            TransactionAction::GetStagedCount => "get_staged_count",
            TransactionAction::GetTransientCount => "get_transient_count",
            TransactionAction::NewHolon { .. } => "new_holon",
            TransactionAction::StageNewHolon { .. } => "stage_new_holon",
            TransactionAction::StageNewFromClone { .. } => "stage_new_from_clone",
            TransactionAction::StageNewVersion { .. } => "stage_new_version",
            TransactionAction::StageNewVersionFromId { .. } => "stage_new_version_from_id",
            TransactionAction::DeleteHolon { .. } => "delete_holon",
        }
    }
}
