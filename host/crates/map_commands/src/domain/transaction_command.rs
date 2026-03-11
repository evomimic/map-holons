use std::sync::Arc;

use base_types::MapString;
use core_types::{HolonId, LocalId};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceRequest;
use holons_core::query_layer::QueryExpression;
use holons_core::reference_layer::{HolonReference, SmartReference, TransientReference};

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
#[derive(Debug)]
pub enum TransactionAction {
    /// Commits the transaction.
    Commit,

    /// Loads holons from a content bundle.
    LoadHolons { bundle: HolonReference },

    /// Executes a dance request within this transaction.
    Dance(DanceRequest),

    /// Executes a lookup action within this transaction.
    Lookup(LookupAction),

    /// Executes a mutation action within this transaction.
    Mutation(MutationAction),

    /// Executes a query expression within this transaction.
    Query(QueryExpression),
}

/// Domain-level lookup actions.
///
/// Maps 1:1 to the `LookupFacade` methods in
/// `shared_crates/holons_core/src/core_shared_objects/transactions/lookup_facade.rs`.
#[derive(Debug)]
pub enum LookupAction {
    /// `LookupFacade::get_all_holons()` → `HolonCollection`
    GetAllHolons,

    /// `LookupFacade::get_staged_holon_by_base_key(key)` → `StagedReference`
    GetStagedHolonByBaseKey { key: MapString },

    /// `LookupFacade::get_staged_holons_by_base_key(key)` → `Vec<StagedReference>`
    GetStagedHolonsByBaseKey { key: MapString },

    /// `LookupFacade::get_staged_holon_by_versioned_key(key)` → `StagedReference`
    GetStagedHolonByVersionedKey { key: MapString },

    /// `LookupFacade::get_transient_holon_by_base_key(key)` → `TransientReference`
    GetTransientHolonByBaseKey { key: MapString },

    /// `LookupFacade::get_transient_holon_by_versioned_key(key)` → `TransientReference`
    GetTransientHolonByVersionedKey { key: MapString },

    /// `LookupFacade::staged_count()` → `i64`
    StagedCount,

    /// `LookupFacade::transient_count()` → `i64`
    TransientCount,
}

/// Domain-level mutation actions.
///
/// Maps 1:1 to the `MutationFacade` methods in
/// `shared_crates/holons_core/src/core_shared_objects/transactions/mutation_facade.rs`.
#[derive(Debug)]
pub enum MutationAction {
    /// `MutationFacade::new_holon(key)` → `TransientReference`
    NewHolon { key: Option<MapString> },

    /// `MutationFacade::stage_new_holon(transient_reference)` → `StagedReference`
    StageNewHolon { source: TransientReference },

    /// `MutationFacade::stage_new_from_clone(original, new_key)` → `StagedReference`
    StageNewFromClone {
        original: HolonReference,
        new_key: MapString,
    },

    /// `MutationFacade::stage_new_version(current_version)` → `StagedReference`
    StageNewVersion { current_version: SmartReference },

    /// `MutationFacade::stage_new_version_from_id(holon_id)` → `StagedReference`
    StageNewVersionFromId { holon_id: HolonId },

    /// `MutationFacade::delete_holon(local_id)` → `()`
    DeleteHolon { local_id: LocalId },
}
