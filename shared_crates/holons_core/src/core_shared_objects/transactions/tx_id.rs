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
}
