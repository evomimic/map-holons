//! SQLite-backed local transaction recovery store.
//!
//! Two tables:
//!   `recovery_session`    — one row per open transaction (envelope + stack pointers)
//!   `recovery_checkpoint` — one row per undo/redo checkpoint (snapshot blob)
//!
//! The schema is embedded as a string constant and applied on `new()`.

use std::{path::Path, sync::Arc};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde_json;
use uuid::Uuid;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use crate::RecoveryStore;

use super::transaction_snapshot::{TransactionSnapshot, now_ms};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Embedded schema — self-contained, no external migration files
    // -----------------------------------------------------------------------

    const SCHEMA_SQL: &'static str = "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS recovery_session (
            tx_id                 TEXT PRIMARY KEY,
            lifecycle_state       TEXT NOT NULL DEFAULT 'Open',
            latest_checkpoint_id  TEXT,
            undo_stack_json       TEXT NOT NULL DEFAULT '[]',
            redo_stack_json       TEXT NOT NULL DEFAULT '[]',
            format_version        INTEGER NOT NULL DEFAULT 1,
            updated_at_ms         INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS recovery_checkpoint (
            checkpoint_id   TEXT    PRIMARY KEY,
            tx_id           TEXT    NOT NULL,
            stack_kind      TEXT    NOT NULL CHECK (stack_kind IN ('undo', 'redo')),
            stack_pos       INTEGER NOT NULL,
            snapshot_blob   BLOB    NOT NULL,
            snapshot_hash   TEXT,
            description     TEXT,
            disable_undo    INTEGER NOT NULL DEFAULT 0,
            created_at_ms   INTEGER NOT NULL,
            FOREIGN KEY (tx_id)
                REFERENCES recovery_session(tx_id)
                ON DELETE CASCADE
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_checkpoint_stack_pos
            ON recovery_checkpoint(tx_id, stack_kind, stack_pos);

        CREATE INDEX IF NOT EXISTS idx_checkpoint_tx_created
            ON recovery_checkpoint(tx_id, created_at_ms);
    ";


pub struct TransactionRecoveryStore {
    conn: Mutex<Connection>,
}

impl RecoveryStore for TransactionRecoveryStore {
    /// Open (or create) the SQLite recovery store at `path`.
    /// Applies the embedded schema — idempotent, safe to call on existing DBs.
    fn new(path: &Path) -> Result<Self, HolonError> {
        let conn = Connection::open(path)
            .map_err(|e| HolonError::Misc(format!("SQLite open failed at {path:?}: {e}")))?;

        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| HolonError::Misc(format!("Schema init failed: {e}")))?;

        tracing::debug!("[RECOVERY STORE] Ready at {path:?}");
        Ok(Self { conn: Mutex::new(conn) })
    }

    // -----------------------------------------------------------------------
    // Persist — called after every successful command
    // -----------------------------------------------------------------------

    /// Capture the current context state and persist a new checkpoint.
    ///
    /// - Builds a `TransactionSnapshot` from the context.
    /// - If `disable_undo` is false: inserts a new undo checkpoint row,
    ///   clears the redo stack, and pushes the checkpoint_id onto the undo stack.
    /// - If `disable_undo` is true: updates the latest recoverable state
    ///   without adding to the undo stack.
    /// - All writes are in a single SQLite transaction (atomic).
    fn persist(
        &self,
        context: &Arc<TransactionContext>,
        description: &str,
        disable_undo: bool,
    ) -> Result<(), HolonError> {
        let snapshot = TransactionSnapshot::from_context(context)?;
        let tx_id = snapshot.tx_id.clone();
        let now = now_ms();
        let checkpoint_id = Uuid::new_v4().to_string();

        let snapshot_blob = serde_json::to_vec(&snapshot)
            .map_err(|e| HolonError::Misc(format!("Serialize snapshot: {e}")))?;

        let mut guard = lock(self)?;

        // Load stacks before opening the write transaction (read-only query).
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, &tx_id)?;

        // Use conn.transaction() so that any error automatically rolls back,
        // preventing a dangling open transaction on the next call.
        let tx = guard
            .transaction()
            .map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        if !disable_undo {
            // New undoable command invalidates all redo history.
            tx.execute(
                "DELETE FROM recovery_checkpoint WHERE tx_id = ?1 AND stack_kind = 'redo'",
                params![tx_id],
            )
            .map_err(|e| HolonError::Misc(format!("Clear redo checkpoints: {e}")))?;
            redo_stack.clear();
            undo_stack.push(checkpoint_id.clone());
        }

        // Compute the final stack JSON and latest pointer before any INSERTs.
        let undo_json = serde_json::to_string(&undo_stack)
            .map_err(|e| HolonError::Misc(format!("Serialize undo stack: {e}")))?;
        let redo_json = serde_json::to_string(&redo_stack)
            .map_err(|e| HolonError::Misc(format!("Serialize redo stack: {e}")))?;

        // ── Step 1: upsert session row FIRST so the FK on recovery_checkpoint is satisfied ──
        tx.execute(
            "INSERT INTO recovery_session
                 (tx_id, lifecycle_state, latest_checkpoint_id,
                  undo_stack_json, redo_stack_json, format_version, updated_at_ms)
             VALUES (?1, 'Open', ?2, ?3, ?4, 1, ?5)
             ON CONFLICT(tx_id) DO UPDATE SET
                 latest_checkpoint_id = excluded.latest_checkpoint_id,
                 undo_stack_json      = excluded.undo_stack_json,
                 redo_stack_json      = excluded.redo_stack_json,
                 updated_at_ms        = excluded.updated_at_ms",
            params![tx_id, checkpoint_id, undo_json, redo_json, now],
        )
        .map_err(|e| HolonError::Misc(format!("Upsert session: {e}")))?;

        // ── Step 2: insert the checkpoint row (FK now satisfied) ──
        if disable_undo {
            // Detached "latest" snapshot — not on the undo stack, used only for
            // crash recovery of the most recent committed state.
            tx.execute(
                "INSERT INTO recovery_checkpoint
                    (checkpoint_id, tx_id, stack_kind, stack_pos,
                     snapshot_blob, snapshot_hash, description, disable_undo, created_at_ms)
                 VALUES (?1, ?2, 'undo', -1, ?3, ?4, ?5, 1, ?6)",
                params![
                    checkpoint_id,
                    tx_id,
                    snapshot_blob,
                    snapshot.hash,
                    description,
                    now,
                ],
            )
            .map_err(|e| HolonError::Misc(format!("Insert no-undo checkpoint: {e}")))?;
        } else {
            let stack_pos = (undo_stack.len() as i64) - 1; // already pushed above
            tx.execute(
                "INSERT INTO recovery_checkpoint
                    (checkpoint_id, tx_id, stack_kind, stack_pos,
                     snapshot_blob, snapshot_hash, description, disable_undo, created_at_ms)
                 VALUES (?1, ?2, 'undo', ?3, ?4, ?5, ?6, 0, ?7)",
                params![
                    checkpoint_id,
                    tx_id,
                    stack_pos,
                    snapshot_blob,
                    snapshot.hash,
                    description,
                    now,
                ],
            )
            .map_err(|e| HolonError::Misc(format!("Insert checkpoint: {e}")))?;
        }

        tx.commit()
            .map_err(|e| HolonError::Misc(format!("Commit transaction: {e}")))?;

        tracing::debug!("[RECOVERY STORE] Persisted checkpoint '{description}' for tx={tx_id}");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Undo — pop from undo stack, restore previous checkpoint
    // -----------------------------------------------------------------------

    /// Pop the top of the undo stack and return the snapshot to restore.
    /// Moves the popped checkpoint to the redo stack.
    /// Returns `None` if nothing to undo.
    fn undo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        let Some(popped_id) = undo_stack.pop() else {
            tracing::debug!("[RECOVERY STORE] Nothing to undo for tx={tx_id}");
            return Ok(None);
        };

        let restore_id = undo_stack.last().cloned();
        let snapshot = match restore_id.as_ref() {
            Some(id) => Some(load_snapshot(&guard, id)?),
            None => None,
        };

        // Move checkpoint to redo stack
        let redo_pos = redo_stack.len() as i64;
        redo_stack.push(popped_id.clone());

        let tx = guard
            .transaction()
            .map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        tx.execute(
            "UPDATE recovery_checkpoint
             SET stack_kind = 'redo', stack_pos = ?1
             WHERE checkpoint_id = ?2",
            params![redo_pos, popped_id],
        )
        .map_err(|e| HolonError::Misc(format!("Undo: move to redo: {e}")))?;

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, now)?;

        tx.commit()
            .map_err(|e| HolonError::Misc(format!("Undo commit: {e}")))?;

        if let Some(restored_id) = restore_id {
            tracing::info!(
                "[RECOVERY STORE] Undo: restored checkpoint '{restored_id}' for tx={tx_id}"
            );
        } else {
            tracing::info!(
                "[RECOVERY STORE] Undo: restored baseline (no prior checkpoint) for tx={tx_id}"
            );
        }
        Ok(snapshot)
    }

    // -----------------------------------------------------------------------
    // Redo — pop from redo stack, restore checkpoint
    // -----------------------------------------------------------------------

    /// Pop the top of the redo stack and return the snapshot to restore.
    /// Moves the checkpoint back to the undo stack.
    /// Returns `None` if nothing to redo.
    fn redo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        let Some(checkpoint_id) = redo_stack.pop() else {
            tracing::debug!("[RECOVERY STORE] Nothing to redo for tx={tx_id}");
            return Ok(None);
        };

        let snapshot = load_snapshot(&guard, &checkpoint_id)?;

        let undo_pos = undo_stack.len() as i64;
        undo_stack.push(checkpoint_id.clone());

        let tx = guard
            .transaction()
            .map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        tx.execute(
            "UPDATE recovery_checkpoint
             SET stack_kind = 'undo', stack_pos = ?1
             WHERE checkpoint_id = ?2",
            params![undo_pos, checkpoint_id],
        )
        .map_err(|e| HolonError::Misc(format!("Redo: move to undo: {e}")))?;

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, now)?;

        tx.commit()
            .map_err(|e| HolonError::Misc(format!("Redo commit: {e}")))?;

        tracing::info!("[RECOVERY STORE] Redo: restored checkpoint '{checkpoint_id}' for tx={tx_id}");
        Ok(Some(snapshot))
    }

    // -----------------------------------------------------------------------
    // Startup recovery
    // -----------------------------------------------------------------------

    /// Recover the latest consistent snapshot on app startup.
    /// Returns `None` if no recovery data exists for this tx_id.
    /// Verifies the snapshot hash before returning — corrupt snapshots are discarded.
    fn recover_latest(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let conn = lock(self)?;

        let latest_id: Option<String> = conn
            .query_row(
                "SELECT latest_checkpoint_id FROM recovery_session WHERE tx_id = ?1",
                params![tx_id],
                |r| r.get(0),
            )
            .unwrap_or(None);

        let Some(checkpoint_id) = latest_id else {
            tracing::debug!("[RECOVERY STORE] No recovery snapshot for tx={tx_id}");
            return Ok(None);
        };

        let snapshot = load_snapshot(&conn, &checkpoint_id)?;

        // Integrity check before handing back to caller
        snapshot.verify_integrity().map_err(|e| {
            tracing::error!("[RECOVERY STORE] Corrupt snapshot discarded: {e}");
            e
        })?;

        tracing::info!("[RECOVERY STORE] Recovered snapshot for tx={tx_id} from checkpoint='{checkpoint_id}'");
        Ok(Some(snapshot))
    }

    // -----------------------------------------------------------------------
    // Cleanup — on commit or rollback
    // -----------------------------------------------------------------------

    /// Delete ALL recovery state for this transaction (session row + all checkpoints).
    /// The FK ON DELETE CASCADE removes checkpoint rows automatically.
    /// Call on successful commit or explicit rollback.
    fn cleanup(&self, tx_id: &str) -> Result<(), HolonError> {
        let conn = lock(self)?;
        let deleted = conn
            .execute(
                "DELETE FROM recovery_session WHERE tx_id = ?1",
                params![tx_id],
            )
            .map_err(|e| HolonError::Misc(format!("Cleanup failed for tx={tx_id}: {e}")))?;

        tracing::info!(
            "[RECOVERY STORE] Cleaned up recovery state for tx={tx_id} ({deleted} session rows removed)"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Stack inspection (for UI: undo/redo availability)
    // -----------------------------------------------------------------------

    /// Returns `true` if there is at least one undoable checkpoint.
    fn can_undo(&self, tx_id: &str) -> Result<bool, HolonError> {
        let conn = lock(self)?;
        let (undo_stack, _) = load_stacks(&conn, tx_id)?;
        Ok(!undo_stack.is_empty())
    }

    /// Returns `true` if there is at least one redoable checkpoint.
    fn can_redo(&self, tx_id: &str) -> Result<bool, HolonError> {
        let conn = lock(self)?;
        let (_, redo_stack) = load_stacks(&conn, tx_id)?;
        Ok(!redo_stack.is_empty())
    }

    /// Returns the descriptions of all undo checkpoints (oldest first).
    /// Useful for building an undo history list in the UI.
    fn undo_history(&self, tx_id: &str) -> Result<Vec<String>, HolonError> {
        let conn = lock(self)?;
        let mut stmt = conn
            .prepare(
                "SELECT description FROM recovery_checkpoint
                 WHERE tx_id = ?1 AND stack_kind = 'undo' AND stack_pos >= 0
                 ORDER BY stack_pos ASC",
            )
            .map_err(|e| HolonError::Misc(format!("Prepare undo_history: {e}")))?;

        let descriptions: Vec<String> = stmt
            .query_map(params![tx_id], |r| r.get(0))
            .map_err(|e| HolonError::Misc(format!("Query undo_history: {e}")))?
            .filter_map(|r| r.ok())
            .filter_map(|s: Option<String>| s)
            .collect();

        Ok(descriptions)
    }

}

// -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn lock(store:&TransactionRecoveryStore) -> Result<std::sync::MutexGuard<'_, Connection>, HolonError> {
        store.conn
            .lock()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))
    }

    fn load_stacks(
        conn: &Connection,
        tx_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), HolonError> {
        let result = conn.query_row(
            "SELECT undo_stack_json, redo_stack_json FROM recovery_session WHERE tx_id = ?1",
            params![tx_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        );

        match result {
            Ok((undo_json, redo_json)) => {
                let undo: Vec<String> = serde_json::from_str(&undo_json)
                    .map_err(|e| HolonError::Misc(format!("Deserialize undo stack: {e}")))?;
                let redo: Vec<String> = serde_json::from_str(&redo_json)
                    .map_err(|e| HolonError::Misc(format!("Deserialize redo stack: {e}")))?;
                Ok((undo, redo))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((vec![], vec![])),
            Err(e) => Err(HolonError::Misc(format!("Load stacks for tx={tx_id}: {e}"))),
        }
    }

    fn save_stacks(
        conn: &Connection,
        tx_id: &str,
        undo_stack: &[String],
        redo_stack: &[String],
        now: i64,
    ) -> Result<(), HolonError> {
        let undo_json = serde_json::to_string(undo_stack)
            .map_err(|e| HolonError::Misc(format!("Serialize undo stack: {e}")))?;
        let redo_json = serde_json::to_string(redo_stack)
            .map_err(|e| HolonError::Misc(format!("Serialize redo stack: {e}")))?;
        let latest = undo_stack.last().cloned();

        conn.execute(
            "UPDATE recovery_session
             SET undo_stack_json = ?1, redo_stack_json = ?2,
                 latest_checkpoint_id = ?3, updated_at_ms = ?4
             WHERE tx_id = ?5",
            params![undo_json, redo_json, latest, now, tx_id],
        )
        .map_err(|e| HolonError::Misc(format!("Save stacks for tx={tx_id}: {e}")))?;

        Ok(())
    }

    fn load_snapshot(
        conn: &Connection,
        checkpoint_id: &str,
    ) -> Result<TransactionSnapshot, HolonError> {
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT snapshot_blob FROM recovery_checkpoint WHERE checkpoint_id = ?1",
                params![checkpoint_id],
                |r| r.get(0),
            )
            .map_err(|e| HolonError::Misc(format!("Load snapshot '{checkpoint_id}': {e}")))?;

        serde_json::from_slice(&blob)
            .map_err(|e| HolonError::Misc(format!("Deserialize snapshot '{checkpoint_id}': {e}")))
    }