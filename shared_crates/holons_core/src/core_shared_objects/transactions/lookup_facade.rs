use std::sync::Arc;

use super::TransactionContext;

/// Semantic facade for transaction-scoped lookup operations.
#[derive(Debug, Clone)]
pub struct LookupFacade {
    pub(crate) ctx: Arc<TransactionContext>,
}
