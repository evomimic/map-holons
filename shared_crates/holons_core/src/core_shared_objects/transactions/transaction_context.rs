//! Transaction-scoped execution context (structure only).

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};

use core_types::HolonError;

use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::{Nursery, TransientHolonManager};

use super::TxId;

/// Transaction-scoped execution context.
#[derive(Debug)]
pub struct TransactionContext {
    tx_id: TxId,
    is_open: AtomicBool,
    space_manager: Weak<HolonSpaceManager>,
    nursery: Nursery,
    transient_manager: TransientHolonManager,
}

impl TransactionContext {
    /// Creates a new transaction context with its own staging and transient pools.
    pub fn new(tx_id: TxId, space_manager: Weak<HolonSpaceManager>) -> Self {
        Self {
            tx_id,
            is_open: AtomicBool::new(true),
            space_manager,
            nursery: Nursery::new(),
            transient_manager: TransientHolonManager::new_empty(),
        }
    }

    /// Returns the transaction id.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Returns whether the transaction is still open.
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Acquire)
    }

    /// Returns a strong reference to the space manager, if it is still alive.
    pub fn space_manager(&self) -> Result<Arc<HolonSpaceManager>, HolonError> {
        self.space_manager
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("HolonSpaceManager".into()))
    }

    /// Provides access to the transaction-owned nursery.
    pub fn nursery(&self) -> &Nursery {
        &self.nursery
    }

    /// Provides access to the transaction-owned transient manager.
    pub fn transient_manager(&self) -> &TransientHolonManager {
        &self.transient_manager
    }
}
