//! Snapshot types for local crash recovery and undo/redo.
//!
//! These are LOCAL-ONLY structures — not persistent holons, not wire types
//! for IPC. They exist solely to capture and restore transaction graph state
//! via the local recovery store.

use holons_boundary::session_state::SerializableHolonPool;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::HolonError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One point in the undo/redo timeline.
///
/// Stored as a row in `recovery_checkpoint`. The full graph state
/// (staged + transient pools) is captured at the moment a command succeeds.
/// LOCAL-ONLY — never crosses a wire boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoCheckpoint {
    /// UUID uniquely identifying this checkpoint row.
    pub checkpoint_id: String,

    /// Human-readable label from the command that triggered this checkpoint
    /// (e.g. "stage_new_holon", "with_property"). Used for undo/redo UI labels.
    pub description: String,

    /// Whether this checkpoint can be undone.
    /// `true` for bulk/loader operations that shouldn't appear in the undo stack.
    pub disable_undo: bool,

    /// Full staged holon pool at this point in time.
    pub staged_holons: SerializableHolonPool,

    /// Full transient holon pool at this point in time.
    pub transient_holons: SerializableHolonPool,

    /// Unix timestamp (ms) when this checkpoint was created.
    pub timestamp: i64,
}

/// Complete transaction graph state — the unit persisted per checkpoint.
///
/// This is what gets serialized into `snapshot_blob` in `recovery_checkpoint`,
/// and also what gets restored into a `TransactionContext` on undo/redo/recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSnapshot {
    /// String form of TxId for storage — ephemeral per session.
    pub tx_id: String,

    /// Unix timestamp (ms) when this snapshot was taken.
    pub timestamp: i64,

    /// Staged holon pool serialized via the existing wire path.
    pub staged_holons: SerializableHolonPool,

    /// Transient holon pool serialized via the existing wire path.
    pub transient_holons: SerializableHolonPool,

    /// SHA-256 hex digest of the serialized pools — for integrity checking on restore.
    /// Computed from the JSON bytes of staged + transient at snapshot time.
    pub hash: String,
}

impl TransactionSnapshot {
    /// Capture the current transaction graph state.
    ///
    /// Uses the same export path as `DanceEnvelopeAdapter::attach_session_state`
    /// — `export_staged_holons` / `export_transient_holons` — which are the
    /// authoritative, WASM-safe export methods on `TransactionContext`.
    pub fn from_context(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        let staged_pool = context.export_staged_holons()?;
        let transient_pool = context.export_transient_holons()?;

        let staged_holons = SerializableHolonPool::from(&staged_pool);
        let transient_holons = SerializableHolonPool::from(&transient_pool);

        // Compute hash from serialized pools for integrity verification on restore.
        let hash = Self::compute_hash(&staged_holons, &transient_holons)?;

        Ok(Self {
            tx_id: context.tx_id().value().to_string(),
            timestamp: now_ms(),
            staged_holons,
            transient_holons,
            hash,
        })
    }

    /// Verify the stored hash against the current pool contents.
    /// Returns `Ok(())` if they match, `Err` if the snapshot is corrupt.
    pub fn verify_integrity(&self) -> Result<(), HolonError> {
        let expected = Self::compute_hash(&self.staged_holons, &self.transient_holons)?;
        if expected == self.hash {
            Ok(())
        } else {
            Err(HolonError::Misc(format!(
                "Snapshot integrity check failed for tx_id={}: hash mismatch \
                 (stored={}, computed={})",
                self.tx_id, self.hash, expected
            )))
        }
    }

    fn compute_hash(
        staged: &SerializableHolonPool,
        transient: &SerializableHolonPool,
    ) -> Result<String, HolonError> {
        use sha2::{Digest, Sha256};

        let staged_bytes = serde_json::to_vec(staged)
            .map_err(|e| HolonError::Misc(format!("Hash: staged serialize failed: {e}")))?;
        let transient_bytes = serde_json::to_vec(transient)
            .map_err(|e| HolonError::Misc(format!("Hash: transient serialize failed: {e}")))?;

        let mut hasher = Sha256::new();
        hasher.update(&staged_bytes);
        hasher.update(&transient_bytes);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}