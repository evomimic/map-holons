//! Transaction manager authority for transaction creation and registration.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use core_types::HolonError;

use crate::core_shared_objects::space_manager::HolonSpaceManager;

use super::tx_id::TransactionIdGenerator;
use super::{TransactionContext, TxId};

/// Per-space transaction manager.
#[derive(Debug)]
pub struct TransactionManager {
    id_generator: TransactionIdGenerator,
    transactions: RwLock<HashMap<TxId, Arc<TransactionContext>>>,
}

impl TransactionManager {
    /// Creates a new transaction manager with an empty registry.
    pub fn new() -> Self {
        Self {
            id_generator: TransactionIdGenerator::new(),
            transactions: RwLock::new(HashMap::new()),
        }
    }

    /// Creates and registers the implicit default transaction for this space.
    pub fn open_default_transaction(
        &self,
        space_manager: &Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        self.open_transaction(space_manager)
    }

    /// Looks up a transaction by id.
    pub fn get_transaction(
        &self,
        tx_id: &TxId,
    ) -> Result<Option<Arc<TransactionContext>>, HolonError> {
        let guard = self.transactions.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transactions: {}",
                e
            ))
        })?;
        let transaction = guard.get(tx_id).map(Arc::clone);
        drop(guard);
        Ok(transaction)
    }

    fn open_transaction(
        &self,
        space_manager: &Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        let tx_id = self.id_generator.next_id();
        let context = Arc::new(TransactionContext::new(tx_id, Arc::downgrade(space_manager)));

        let mut guard = self.transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on transactions: {}",
                e
            ))
        })?;
        guard.insert(tx_id, Arc::clone(&context));
        drop(guard);

        Ok(context)
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
