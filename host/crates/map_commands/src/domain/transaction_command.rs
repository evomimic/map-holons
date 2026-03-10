use std::sync::Arc;

use base_types::MapString;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceRequest;
use holons_core::query_layer::QueryExpression;
use holons_core::reference_layer::{HolonReference, TransientReference};

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

    /// Creates a new transient holon (not yet staged).
    CreateTransientHolon { key: Option<MapString> },

    /// Stages a transient holon for commit.
    StageNewHolon { source: TransientReference },

    /// Creates a staged new version of a committed holon.
    StageNewVersion { holon: HolonReference },

    /// Loads holons from a content bundle.
    LoadHolons { bundle: HolonReference },

    /// Executes a dance request within this transaction.
    Dance(DanceRequest),

    /// Executes a lookup query within this transaction.
    Lookup(LookupQuery),
}

/// Domain-level lookup query.
#[derive(Debug)]
pub enum LookupQuery {
    /// Retrieves a single holon by reference.
    GetHolon(HolonReference),

    /// Evaluates a query expression.
    QueryExpression(QueryExpression),

    /// Retrieves all holons in the space.
    GetAllHolons,
}
