use std::sync::Arc;

use super::TransactionContext;

/// Semantic facade for transaction-scoped mutation operations.
#[derive(Debug, Clone)]
pub struct MutationFacade {
    pub(crate) ctx: Arc<TransactionContext>,
}
