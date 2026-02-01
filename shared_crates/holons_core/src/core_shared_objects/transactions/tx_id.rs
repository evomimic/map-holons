//! Transaction identifiers and internal id generation.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Ephemeral transaction identifier (session-local).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxId(u64);

impl TxId {
    /// Returns the raw numeric id for serialization or logging.
    pub fn value(&self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
pub(super) struct TransactionIdGenerator {
    next_id: AtomicU64,
}

impl TransactionIdGenerator {
    pub(super) fn new() -> Self {
        Self { next_id: AtomicU64::new(1) }
    }

    pub(super) fn next_id(&self) -> TxId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        TxId(id)
    }

    // Ensures no TxId collisions after creating a TransactionContext for a given TxId.
    pub(super) fn bump_to_at_least(&self, tx_id: TxId) {
        let target = tx_id.value().saturating_add(1);
        let mut current = self.next_id.load(Ordering::Relaxed);

        while current < target {
            match self.next_id.compare_exchange(
                current,
                target,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }
}
