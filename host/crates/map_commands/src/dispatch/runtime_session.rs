use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use core_types::HolonError;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};

/// Transaction ownership layer for MAP Commands.
///
/// Holds strong `Arc<TransactionContext>` references for all active transactions.
/// `TransactionManager` remains weak-registry/id-authority only (per issue 370).
///
/// Follow-up: once the `ClientSession` PR 418 merges, this should store
/// `ClientSession` values instead of raw `Arc<TransactionContext>` to get
/// undo/redo/recovery for free.
pub struct RuntimeSession {
    space_manager: Arc<HolonSpaceManager>,
    active_transactions: RwLock<HashMap<TxId, Arc<TransactionContext>>>,
}

impl RuntimeSession {
    pub fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager, active_transactions: RwLock::new(HashMap::new()) }
    }

    /// Opens a new transaction via the space's TransactionManager and stores
    /// the strong reference in `active_transactions`.
    pub fn begin_transaction(&self) -> Result<TxId, HolonError> {
        let context = self
            .space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&self.space_manager))?;

        let tx_id = context.tx_id();

        let mut guard = self.active_transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_transactions: {}",
                e
            ))
        })?;
        guard.insert(tx_id, context);

        Ok(tx_id)
    }

    /// Looks up an active transaction by TxId.
    ///
    /// Returns an error if the transaction is not found (expired or never created).
    pub fn get_transaction(&self, tx_id: &TxId) -> Result<Arc<TransactionContext>, HolonError> {
        let guard = self.active_transactions.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on active_transactions: {}",
                e
            ))
        })?;

        guard.get(tx_id).cloned().ok_or_else(|| {
            HolonError::InvalidParameter(format!(
                "No active transaction for tx_id={}",
                tx_id.value()
            ))
        })
    }

    /// Removes a transaction from active ownership (e.g., after commit or abandon).
    pub fn remove_transaction(&self, tx_id: &TxId) -> Result<(), HolonError> {
        let mut guard = self.active_transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_transactions: {}",
                e
            ))
        })?;
        guard.remove(tx_id);
        Ok(())
    }

    /// Returns a reference to the space manager.
    pub fn space_manager(&self) -> &Arc<HolonSpaceManager> {
        &self.space_manager
    }
}

impl std::fmt::Debug for RuntimeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tx_count = self.active_transactions.read().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("RuntimeSession").field("active_transactions", &tx_count).finish()
    }
}
