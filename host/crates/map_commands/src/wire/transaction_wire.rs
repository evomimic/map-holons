use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use holons_boundary::{DanceRequestWire, HolonReferenceWire, TransientReferenceWire};
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::query_layer::QueryExpression;
use serde::{Deserialize, Serialize};

use crate::domain::{LookupQuery, TransactionAction, TransactionCommand};

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

    /// Creates a new transient holon (not yet staged).
    CreateTransientHolon { key: Option<MapString> },

    /// Stages a transient holon for commit.
    StageNewHolon { source: TransientReferenceWire },

    /// Creates a staged new version of a committed holon.
    StageNewVersion { holon: HolonReferenceWire },

    /// Loads holons from a content bundle.
    LoadHolons { bundle: HolonReferenceWire },

    /// Executes a dance request within this transaction.
    Dance(DanceRequestWire),

    /// Executes a lookup query within this transaction.
    Lookup(LookupQueryWire),
}

/// Wire-level lookup query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LookupQueryWire {
    /// Retrieves a single holon by reference.
    GetHolon(HolonReferenceWire),

    /// Evaluates a query expression.
    QueryExpression(QueryExpression),

    /// Retrieves all holons in the space.
    GetAllHolons,
}

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
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<TransactionAction, HolonError> {
        match self {
            TransactionActionWire::Commit => Ok(TransactionAction::Commit),
            TransactionActionWire::CreateTransientHolon { key } => {
                Ok(TransactionAction::CreateTransientHolon { key })
            }
            TransactionActionWire::StageNewHolon { source } => {
                Ok(TransactionAction::StageNewHolon {
                    source: source.bind(context)?,
                })
            }
            TransactionActionWire::StageNewVersion { holon } => {
                Ok(TransactionAction::StageNewVersion {
                    holon: holon.bind(context)?,
                })
            }
            TransactionActionWire::LoadHolons { bundle } => {
                Ok(TransactionAction::LoadHolons {
                    bundle: bundle.bind(context)?,
                })
            }
            TransactionActionWire::Dance(request_wire) => {
                Ok(TransactionAction::Dance(request_wire.bind(context)?))
            }
            TransactionActionWire::Lookup(query_wire) => {
                Ok(TransactionAction::Lookup(query_wire.bind(context)?))
            }
        }
    }
}

impl LookupQueryWire {
    fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<LookupQuery, HolonError> {
        match self {
            LookupQueryWire::GetHolon(ref_wire) => {
                Ok(LookupQuery::GetHolon(ref_wire.bind(context)?))
            }
            LookupQueryWire::QueryExpression(expr) => {
                Ok(LookupQuery::QueryExpression(expr))
            }
            LookupQueryWire::GetAllHolons => Ok(LookupQuery::GetAllHolons),
        }
    }
}
