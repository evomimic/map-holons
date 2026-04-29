//! SQLite-backed local transaction recovery store.
//!
//! Two tables:
//!   `recovery_session`    — one row per open transaction (envelope + stack pointers)
//!   `recovery_checkpoint` — one row per undo/redo checkpoint (snapshot blob)
//!
//! The schema is embedded as a string constant and applied on `new()`.

use std::sync::Mutex;
use std::{path::Path, sync::Arc};

use rusqlite::{params, Connection};
use serde_json;
use uuid::Uuid;

use super::RecoveryStore;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;

use super::transaction_snapshot::{now_ms, TransactionSnapshot};

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
            undo_checkpointing_enabled INTEGER NOT NULL DEFAULT 1,
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

        CREATE TABLE IF NOT EXISTS experience_unit (
            unit_id         TEXT    PRIMARY KEY,
            tx_id           TEXT    NOT NULL,
            marker_id       TEXT,
            marker_label    TEXT,
            checkpoint_id   TEXT    NOT NULL,
            stack_kind      TEXT    NOT NULL CHECK (stack_kind IN ('undo', 'redo')),
            stack_pos       INTEGER NOT NULL,
            created_at_ms   INTEGER NOT NULL,
            FOREIGN KEY (tx_id)         REFERENCES recovery_session(tx_id) ON DELETE CASCADE,
            FOREIGN KEY (checkpoint_id) REFERENCES recovery_checkpoint(checkpoint_id)
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_eu_stack_pos
            ON experience_unit(tx_id, stack_kind, stack_pos);

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
    /// Always writes a crash-recovery snapshot (`latest_checkpoint_id`).
    ///
    /// - `disable_undo=true`: marks `undo_checkpointing_enabled=0` for the
    ///   transaction so no future undo units are created; persists crash-recovery
    ///   row only (`stack_pos=-1`).
    /// - `snapshot_after=true` (and checkpointing still enabled): closes the
    ///   current Experience Unit — inserts a checkpoint + experience_unit row,
    ///   pushes the unit_id onto the undo stack, clears redo history.
    /// - Otherwise (intermediate command): updates `latest_checkpoint_id` only.
    ///
    /// All writes are in a single SQLite transaction (atomic).
    fn persist(
        &self,
        context: &Arc<TransactionContext>,
        description: &str,
        disable_undo: bool,
        snapshot_after: bool,
        marker_id: Option<&str>,
        marker_label: Option<&str>,
    ) -> Result<(), HolonError> {
        let snapshot = TransactionSnapshot::from_context(context)?;
        let tx_id = snapshot.tx_id.clone();
        let now = now_ms();
        let checkpoint_id = Uuid::new_v4().to_string();

        let snapshot_blob = serde_json::to_vec(&snapshot)
            .map_err(|e| HolonError::Misc(format!("Serialize snapshot: {e}")))?;

        let mut guard = lock(self)?;

        // Read undo_checkpointing_enabled + stacks before opening the write tx.
        let checkpointing_enabled = load_checkpointing_enabled(&guard, &tx_id)?;
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, &tx_id)?;

        let tx =
            guard.transaction().map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        // ── Step 1: upsert session row (crash-recovery pointer always updated) ──
        // undo_checkpointing_enabled starts as a no-update; we patch it below if needed.
        tx.execute(
            "INSERT INTO recovery_session
                 (tx_id, lifecycle_state, latest_checkpoint_id,
                  undo_stack_json, redo_stack_json,
                  undo_checkpointing_enabled, format_version, updated_at_ms)
             VALUES (?1, 'Open', ?2, '[]', '[]', 1, 1, ?3)
             ON CONFLICT(tx_id) DO UPDATE SET
                 latest_checkpoint_id = excluded.latest_checkpoint_id,
                 updated_at_ms        = excluded.updated_at_ms",
            params![tx_id, checkpoint_id, now],
        )
        .map_err(|e| HolonError::Misc(format!("Upsert session: {e}")))?;

        // ── Step 2: insert the checkpoint blob (FK now satisfied) ──
        // INSERT OR REPLACE so the crash-recovery sentinel (stack_pos=-1) is always
        // replaced by the latest snapshot rather than causing a UNIQUE conflict.
        tx.execute(
            "INSERT OR REPLACE INTO recovery_checkpoint
                (checkpoint_id, tx_id, stack_kind, stack_pos,
                 snapshot_blob, snapshot_hash, description, disable_undo, created_at_ms)
             VALUES (?1, ?2, 'undo', -1, ?3, ?4, ?5, ?6, ?7)",
            params![
                checkpoint_id,
                tx_id,
                snapshot_blob,
                snapshot.hash,
                description,
                disable_undo as i64,
                now,
            ],
        )
        .map_err(|e| HolonError::Misc(format!("Insert checkpoint: {e}")))?;

        // ── Step 3: apply undo semantics ──

        // Any forward mutation diverges from the redo timeline. Clear redo unconditionally —
        // applies to EU-closing, intermediate, and disable_undo mutations alike.
        if !redo_stack.is_empty() {
            tx.execute(
                "DELETE FROM experience_unit WHERE tx_id = ?1 AND stack_kind = 'redo'",
                params![tx_id],
            )
            .map_err(|e| HolonError::Misc(format!("Clear redo EUs on forward mutation: {e}")))?;
            tx.execute(
                "DELETE FROM recovery_checkpoint WHERE tx_id = ?1 AND stack_kind = 'redo'",
                params![tx_id],
            )
            .map_err(|e| {
                HolonError::Misc(format!("Clear redo checkpoints on forward mutation: {e}"))
            })?;
            tx.execute(
                "UPDATE recovery_session SET redo_stack_json = '[]' WHERE tx_id = ?1",
                params![tx_id],
            )
            .map_err(|e| {
                HolonError::Misc(format!("Clear redo stack json on forward mutation: {e}"))
            })?;
            redo_stack.clear();
        }

        if disable_undo {
            // Permanently disable future undo checkpoint creation for this tx.
            tx.execute(
                "UPDATE recovery_session SET undo_checkpointing_enabled = 0 WHERE tx_id = ?1",
                params![tx_id],
            )
            .map_err(|e| HolonError::Misc(format!("Disable checkpointing: {e}")))?;

            tracing::debug!("[RECOVERY STORE] disable_undo: checkpointing disabled for tx={tx_id}");
        } else if snapshot_after && checkpointing_enabled {
            // Close the current Experience Unit: create a checkpoint + EU row,
            // push unit_id onto undo stack.
            let unit_id = Uuid::new_v4().to_string();
            let stack_pos = undo_stack.len() as i64;

            // Update the checkpoint to reflect its undo stack position.
            tx.execute(
                "UPDATE recovery_checkpoint SET stack_pos = ?1 WHERE checkpoint_id = ?2",
                params![stack_pos, checkpoint_id],
            )
            .map_err(|e| HolonError::Misc(format!("Update checkpoint stack_pos: {e}")))?;

            // Insert the Experience Unit record.
            tx.execute(
                "INSERT INTO experience_unit
                    (unit_id, tx_id, marker_id, marker_label,
                     checkpoint_id, stack_kind, stack_pos, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'undo', ?6, ?7)",
                params![unit_id, tx_id, marker_id, marker_label, checkpoint_id, stack_pos, now],
            )
            .map_err(|e| HolonError::Misc(format!("Insert experience_unit: {e}")))?;

            undo_stack.push(unit_id.clone());
            save_stacks(&tx, &tx_id, &undo_stack, &redo_stack, Some(&checkpoint_id), now)?;

            tracing::debug!(
                "[RECOVERY STORE] Closed ExperienceUnit unit_id={unit_id} \
                 checkpoint={checkpoint_id} for tx={tx_id}"
            );
        }
        // Else: intermediate command — crash recovery only (session already updated in step 1).

        tx.commit().map_err(|e| HolonError::Misc(format!("Commit transaction: {e}")))?;

        tracing::debug!("[RECOVERY STORE] Persisted checkpoint '{description}' for tx={tx_id}");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Undo — pop top ExperienceUnit, restore the checkpoint before it
    // -----------------------------------------------------------------------

    /// Pop the top ExperienceUnit from the undo stack and return the snapshot
    /// that preceded it (i.e. the checkpoint of the unit now at the top after
    /// the pop, or `None` for baseline).
    /// Moves the popped unit to the redo stack.
    /// Returns `None` if the undo stack is empty.
    fn undo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        let Some(popped_unit_id) = undo_stack.pop() else {
            tracing::debug!("[RECOVERY STORE] Nothing to undo for tx={tx_id}");
            return Ok(None);
        };

        // Pre-compute restore target and latest checkpoint pointer before opening
        // the write transaction — guard can't be borrowed immutably once tx is open.
        let popped_checkpoint_id = load_checkpoint_for_unit(&guard, &popped_unit_id)?;
        let (snapshot, latest_cp) = match undo_stack.last() {
            Some(prior_unit_id) => {
                let cp_id = load_checkpoint_for_unit(&guard, prior_unit_id)?;
                let snap = load_snapshot(&guard, &cp_id)?;
                (Some(snap), Some(cp_id))
            }
            None => (None, None),
        };

        let redo_pos = redo_stack.len() as i64;
        redo_stack.push(popped_unit_id.clone());

        let tx =
            guard.transaction().map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        // Move the experience_unit to the redo stack.
        tx.execute(
            "UPDATE experience_unit
             SET stack_kind = 'redo', stack_pos = ?1
             WHERE unit_id = ?2",
            params![redo_pos, popped_unit_id],
        )
        .map_err(|e| HolonError::Misc(format!("Undo: move EU to redo: {e}")))?;

        // Keep recovery_checkpoint in sync so its (tx_id, stack_kind, stack_pos)
        // index stays consistent with the experience_unit position.
        tx.execute(
            "UPDATE recovery_checkpoint
             SET stack_kind = 'redo', stack_pos = ?1
             WHERE checkpoint_id = ?2",
            params![redo_pos, popped_checkpoint_id],
        )
        .map_err(|e| HolonError::Misc(format!("Undo: sync checkpoint to redo: {e}")))?;

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, latest_cp.as_deref(), now)?;

        tx.commit().map_err(|e| HolonError::Misc(format!("Undo commit: {e}")))?;

        tracing::info!("[RECOVERY STORE] Undo: popped unit={popped_unit_id} for tx={tx_id}");
        Ok(snapshot)
    }

    // -----------------------------------------------------------------------
    // Redo — pop top ExperienceUnit from redo, restore its checkpoint
    // -----------------------------------------------------------------------

    /// Pop the top ExperienceUnit from the redo stack and return its snapshot.
    /// Moves the unit back to the undo stack.
    /// Returns `None` if the redo stack is empty.
    fn redo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        let Some(unit_id) = redo_stack.pop() else {
            tracing::debug!("[RECOVERY STORE] Nothing to redo for tx={tx_id}");
            return Ok(None);
        };

        let checkpoint_id = load_checkpoint_for_unit(&guard, &unit_id)?;
        let snapshot = load_snapshot(&guard, &checkpoint_id)?;

        let undo_pos = undo_stack.len() as i64;
        undo_stack.push(unit_id.clone());

        let tx =
            guard.transaction().map_err(|e| HolonError::Misc(format!("Begin transaction: {e}")))?;

        // Move the experience_unit back to the undo stack.
        tx.execute(
            "UPDATE experience_unit
             SET stack_kind = 'undo', stack_pos = ?1
             WHERE unit_id = ?2",
            params![undo_pos, unit_id],
        )
        .map_err(|e| HolonError::Misc(format!("Redo: move EU to undo: {e}")))?;

        // Keep recovery_checkpoint in sync with the experience_unit position.
        tx.execute(
            "UPDATE recovery_checkpoint
             SET stack_kind = 'undo', stack_pos = ?1
             WHERE checkpoint_id = ?2",
            params![undo_pos, checkpoint_id],
        )
        .map_err(|e| HolonError::Misc(format!("Redo: sync checkpoint to undo: {e}")))?;

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, Some(&checkpoint_id), now)?;

        tx.commit().map_err(|e| HolonError::Misc(format!("Redo commit: {e}")))?;

        tracing::info!("[RECOVERY STORE] Redo: restored unit={unit_id} for tx={tx_id}");
        Ok(Some(snapshot))
    }

    /// Pop ExperienceUnits from the undo stack until the unit with `marker_id` is popped
    /// (inclusive). All popped units are moved to the redo stack. Returns the snapshot
    /// representing the state just before the marked unit, or `None` for baseline.
    /// Returns `Err(InvalidParameter)` if the marker is not reachable on the undo stack.
    fn undo_to_marker(
        &self,
        tx_id: &str,
        marker_id: &str,
    ) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        // Load EU list newest-first; all reads happen before the write transaction opens.
        let mut undo_units = load_eu_stack(&guard, tx_id, "undo")?;
        // undo_units[0] = top of stack (newest), same order as undo_stack reversed.

        let marker_pos = undo_units
            .iter()
            .position(|(_, _, mid)| mid.as_deref() == Some(marker_id))
            .ok_or_else(|| {
                HolonError::InvalidParameter(format!(
                    "marker_id '{marker_id}' not found on undo stack for tx={tx_id}"
                ))
            })?;

        // Units to pop: indices 0..=marker_pos (newest-first, inclusive of the marker).
        let to_pop: Vec<(String, String, Option<String>)> =
            undo_units.drain(..=marker_pos).collect();

        // Restore target = checkpoint of the new undo top (the unit just below the marker).
        let (restore_snapshot, latest_cp) = match undo_units.first() {
            Some((_, cp_id, _)) => {
                let snap = load_snapshot(&guard, cp_id)?;
                (Some(snap), Some(cp_id.clone()))
            }
            None => (None, None),
        };

        let initial_redo_len = redo_stack.len() as i64;
        for (uid, _, _) in &to_pop {
            undo_stack.pop();
            redo_stack.push(uid.clone());
        }

        let tx = guard
            .transaction()
            .map_err(|e| HolonError::Misc(format!("undo_to_marker begin tx: {e}")))?;

        for (i, (uid, cp_id, _)) in to_pop.iter().enumerate() {
            let redo_pos = initial_redo_len + i as i64;
            tx.execute(
                "UPDATE experience_unit SET stack_kind = 'redo', stack_pos = ?1 WHERE unit_id = ?2",
                params![redo_pos, uid],
            )
            .map_err(|e| HolonError::Misc(format!("undo_to_marker: move EU to redo: {e}")))?;
            tx.execute(
                "UPDATE recovery_checkpoint SET stack_kind = 'redo', stack_pos = ?1 WHERE checkpoint_id = ?2",
                params![redo_pos, cp_id],
            )
            .map_err(|e| HolonError::Misc(format!("undo_to_marker: sync checkpoint to redo: {e}")))?;
        }

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, latest_cp.as_deref(), now)?;
        tx.commit().map_err(|e| HolonError::Misc(format!("undo_to_marker commit: {e}")))?;

        tracing::info!(
            "[RECOVERY STORE] undo_to_marker: popped {} units to reach marker='{marker_id}' for tx={tx_id}",
            to_pop.len()
        );
        Ok(restore_snapshot)
    }

    /// Pop ExperienceUnits from the redo stack until the unit with `marker_id` is popped
    /// (inclusive). All popped units are moved back to the undo stack. Returns the snapshot
    /// captured by the marked unit (the state after it was applied).
    /// Returns `Err(InvalidParameter)` if the marker is not reachable on the redo stack.
    fn redo_to_marker(
        &self,
        tx_id: &str,
        marker_id: &str,
    ) -> Result<Option<TransactionSnapshot>, HolonError> {
        let mut guard = lock(self)?;
        let now = now_ms();
        let (mut undo_stack, mut redo_stack) = load_stacks(&guard, tx_id)?;

        // Load redo EUs newest-first (highest stack_pos first).
        let mut redo_units = load_eu_stack(&guard, tx_id, "redo")?;

        let marker_pos = redo_units
            .iter()
            .position(|(_, _, mid)| mid.as_deref() == Some(marker_id))
            .ok_or_else(|| {
                HolonError::InvalidParameter(format!(
                    "marker_id '{marker_id}' not found on redo stack for tx={tx_id}"
                ))
            })?;

        // Drain newest-first up to and including the marker.
        let to_redo: Vec<(String, String, Option<String>)> =
            redo_units.drain(..=marker_pos).collect();

        // The marker is the last entry in to_redo (oldest of those being redone).
        // Its checkpoint represents the state we want to restore.
        let (_, marker_cp, _) = &to_redo[marker_pos];
        let restore_snapshot = load_snapshot(&guard, marker_cp)?;
        let latest_cp = marker_cp.clone();

        let initial_undo_len = undo_stack.len() as i64;
        for (uid, _, _) in &to_redo {
            redo_stack.pop();
            undo_stack.push(uid.clone());
        }

        let tx = guard
            .transaction()
            .map_err(|e| HolonError::Misc(format!("redo_to_marker begin tx: {e}")))?;

        for (i, (uid, cp_id, _)) in to_redo.iter().enumerate() {
            // Newest unit gets the highest undo position; marker gets initial_undo_len.
            let undo_pos = initial_undo_len + (to_redo.len() - 1 - i) as i64;
            tx.execute(
                "UPDATE experience_unit SET stack_kind = 'undo', stack_pos = ?1 WHERE unit_id = ?2",
                params![undo_pos, uid],
            )
            .map_err(|e| HolonError::Misc(format!("redo_to_marker: move EU to undo: {e}")))?;
            tx.execute(
                "UPDATE recovery_checkpoint SET stack_kind = 'undo', stack_pos = ?1 WHERE checkpoint_id = ?2",
                params![undo_pos, cp_id],
            )
            .map_err(|e| HolonError::Misc(format!("redo_to_marker: sync checkpoint to undo: {e}")))?;
        }

        save_stacks(&tx, tx_id, &undo_stack, &redo_stack, Some(&latest_cp), now)?;
        tx.commit().map_err(|e| HolonError::Misc(format!("redo_to_marker commit: {e}")))?;

        tracing::info!(
            "[RECOVERY STORE] redo_to_marker: restored {} units to reach marker='{marker_id}' for tx={tx_id}",
            to_redo.len()
        );
        Ok(Some(restore_snapshot))
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

        tracing::info!(
            "[RECOVERY STORE] Recovered snapshot for tx={tx_id} from checkpoint='{checkpoint_id}'"
        );
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
            .execute("DELETE FROM recovery_session WHERE tx_id = ?1", params![tx_id])
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

    fn list_open_sessions(&self) -> Result<Vec<String>, HolonError> {
        let conn = lock(self)?;
        let mut stmt = conn
            .prepare(
                "SELECT tx_id FROM recovery_session 
                WHERE lifecycle_state = 'Open'
                ORDER BY updated_at_ms DESC",
            )
            .map_err(|e| HolonError::Misc(format!("Prepare list_open_sessions: {e}")))?;

        let sessions: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| HolonError::Misc(format!("Query list_open_sessions: {e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(sessions)
    }
}

// -----------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------

fn lock(
    store: &TransactionRecoveryStore,
) -> Result<std::sync::MutexGuard<'_, Connection>, HolonError> {
    store.conn.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))
}

fn load_stacks(conn: &Connection, tx_id: &str) -> Result<(Vec<String>, Vec<String>), HolonError> {
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
    latest_checkpoint_id: Option<&str>,
    now: i64,
) -> Result<(), HolonError> {
    let undo_json = serde_json::to_string(undo_stack)
        .map_err(|e| HolonError::Misc(format!("Serialize undo stack: {e}")))?;
    let redo_json = serde_json::to_string(redo_stack)
        .map_err(|e| HolonError::Misc(format!("Serialize redo stack: {e}")))?;

    conn.execute(
        "UPDATE recovery_session
             SET undo_stack_json = ?1, redo_stack_json = ?2,
                 latest_checkpoint_id = ?3, updated_at_ms = ?4
             WHERE tx_id = ?5",
        params![undo_json, redo_json, latest_checkpoint_id, now, tx_id],
    )
    .map_err(|e| HolonError::Misc(format!("Save stacks for tx={tx_id}: {e}")))?;

    Ok(())
}

fn load_checkpointing_enabled(conn: &Connection, tx_id: &str) -> Result<bool, HolonError> {
    let result: rusqlite::Result<i64> = conn.query_row(
        "SELECT undo_checkpointing_enabled FROM recovery_session WHERE tx_id = ?1",
        params![tx_id],
        |r| r.get(0),
    );
    match result {
        Ok(v) => Ok(v != 0),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true), // default for new sessions
        Err(e) => Err(HolonError::Misc(format!("Load checkpointing_enabled for tx={tx_id}: {e}"))),
    }
}

fn load_checkpoint_for_unit(conn: &Connection, unit_id: &str) -> Result<String, HolonError> {
    conn.query_row(
        "SELECT checkpoint_id FROM experience_unit WHERE unit_id = ?1",
        params![unit_id],
        |r| r.get(0),
    )
    .map_err(|e| HolonError::Misc(format!("Load checkpoint for unit '{unit_id}': {e}")))
}

/// Load all ExperienceUnit rows for a given stack (newest-first) as `(unit_id, checkpoint_id, marker_id)`.
fn load_eu_stack(
    conn: &Connection,
    tx_id: &str,
    stack_kind: &str,
) -> Result<Vec<(String, String, Option<String>)>, HolonError> {
    let mut stmt = conn
        .prepare(
            "SELECT unit_id, checkpoint_id, marker_id
             FROM experience_unit
             WHERE tx_id = ?1 AND stack_kind = ?2
             ORDER BY stack_pos DESC",
        )
        .map_err(|e| HolonError::Misc(format!("load_eu_stack prepare: {e}")))?;

    let rows = stmt
        .query_map(params![tx_id, stack_kind], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
        })
        .map_err(|e| HolonError::Misc(format!("load_eu_stack query: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| HolonError::Misc(format!("load_eu_stack collect: {e}")))?;

    Ok(rows)
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
