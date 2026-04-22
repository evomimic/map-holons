use core_types::HolonError;
use std::path::Path;
use std::sync::Arc;

use super::transaction_snapshot::TransactionSnapshot;
use holons_core::core_shared_objects::transactions::TransactionContext;

/// Trait object so BaseReceptor can hold any recovery store implementation.
pub trait RecoveryStore: Send + Sync {
    /// Open (or create) a recovery store at `path`.
    fn new(path: &Path) -> Result<Self, HolonError>
    where
        Self: Sized;

    fn persist(
        &self,
        context: &Arc<TransactionContext>,
        description: &str,
        disable_undo: bool,
    ) -> Result<(), HolonError>;

    fn undo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError>;
    fn redo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError>;
    fn recover_latest(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError>;
    fn cleanup(&self, tx_id: &str) -> Result<(), HolonError>;

    fn can_undo(&self, tx_id: &str) -> Result<bool, HolonError>;
    fn can_redo(&self, tx_id: &str) -> Result<bool, HolonError>;
    fn undo_history(&self, tx_id: &str) -> Result<Vec<String>, HolonError>;
    fn list_open_sessions(&self) -> Result<Vec<String>, HolonError>;
}
