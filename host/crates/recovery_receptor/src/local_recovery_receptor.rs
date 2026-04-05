use core_types::{HolonError};
//use client_shared_types::base_receptor::{BaseReceptor, ReceptorType};
use holons_client::shared_types::base_receptor::{BaseReceptor, ReceptorType};
use holons_core::core_shared_objects::transactions::TransactionContext;
use super::storage::transaction_snapshot::TransactionSnapshot;
use super::storage::{RecoveryStore, TransactionRecoveryStore};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

pub struct LocalRecoveryReceptor {
    receptor_id: String,
    receptor_type: ReceptorType,
    properties: HashMap<String, String>,
    recovery_store: Arc<TransactionRecoveryStore>,
    context: OnceLock<Arc<TransactionContext>>, // OnceLock for one-time initialization
}

/// Implementation of LocalReceptor - local host level - no dancing
impl LocalRecoveryReceptor {
    /// Create a new LocalReceptor, returning Result to handle downcast failures
    pub fn new(base_receptor: BaseReceptor) -> Result<Self, HolonError> {

        // Downcast the stored client into our concrete conductor client
        let client_any =
            base_receptor.client_handler.as_ref().expect("a handler is required for LocalRecoveryReceptor").clone();

        let client_handler = client_any
            .downcast::<TransactionRecoveryStore>() 
            .map_err(|_| HolonError::DowncastFailure(format!(
                "Failed to cast client handler for LocalRecoveryReceptor '{}'",
                base_receptor.receptor_id
        )))?;
        

        Ok(Self {
            receptor_id: base_receptor.receptor_id.clone(),
            receptor_type: base_receptor.receptor_type,
            properties: base_receptor.properties.clone(),
            recovery_store: client_handler,
            context: OnceLock::new(),
            //last_tx_id: Mutex::new(None)
            //recover_session: Mutex::new(recover_session),
        })
    }

    pub fn recover_last_tx_id_from_crash(&self) -> Option<String> {
        let orphaned_tx_ids = match self.recovery_store.list_open_sessions() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!("[RECOVERY] Failed to list open sessions: {e}");
                return None;
            }
        };
        let last_tx_id = orphaned_tx_ids.first().cloned().or_else(|| None);
         // Clean up all older orphaned sessions — they are stale
            for stale_tx_id in orphaned_tx_ids.iter().skip(1) {
                if let Err(e) = self.recovery_store.cleanup(stale_tx_id) {
                    tracing::warn!("[RECOVERY] Failed to clean up stale tx={stale_tx_id}: {e}");
                } else {
                    tracing::info!("[RECOVERY] Cleaned up stale orphaned tx={stale_tx_id}");
                }
            }
        last_tx_id
    }
    
    pub fn init_session(&self, context: Arc<TransactionContext>) {
        self.context.set(context).expect("should not get here");
        self.try_restore_session();
    }


    /// Restores a recovered crash snapshot into the given `TransactionContext`.
    ///
    /// Called once during startup by `init_from_state` after a fresh transaction
    /// has been opened. Returns `true` if a snapshot was found and restored,
    /// `false` if there was nothing to recover.
    fn try_restore_session(&self) -> bool {
        let Some(snapshot) = &self.recover_last_snapshot() else {
            return false;
        };
        let Some(context) = &self.context.get() else {
            return false;
        };
        match snapshot.restore_into(context) {
            Ok(()) => {
                tracing::info!(
                    "[RECOVERY] Crash snapshot tx={} restored into context tx={}",
                    snapshot.tx_id,
                    context.tx_id().value()
                );
                true
            }
            Err(e) => {
                tracing::warn!("[RECOVERY] Failed to restore crash snapshot: {e}");
                false
            }
        }
    }

    /// Called after every successful command.
    /// `description` is the command name — used in undo history display.
    /// `disable_undo` = true for bulk/loader ops that shouldn't be individually undoable.
    pub async fn persist(&self, description: &str, disable_undo: bool) {
        let Some(context) = &self.context.get() else {
            return;
        };
        let store = Arc::clone(&self.recovery_store);
        let context = Arc::clone(&context);
        let description = description.to_string();

        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = store.persist(&context, &description, disable_undo) {
                tracing::warn!("[CLIENT SESSION] Persist failed: {e}");
            }
        })
        .await;
    }

    pub async fn undo(&self) -> Option<TransactionSnapshot> {
        let Some(context) = &self.context.get() else {
            return None;
        };
        let store = Arc::clone(&self.recovery_store);
        let tx_id = context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.undo(&tx_id).ok().flatten())
            .await
            .ok()
            .flatten()
    }

    pub async fn redo(&self) -> Option<TransactionSnapshot> {
        let Some(context) = &self.context.get() else {
            return None;
        };
        let store = Arc::clone(&self.recovery_store);
        let tx_id = context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.redo(&tx_id).ok().flatten())
            .await
            .ok()
            .flatten()
    }

    pub async fn list_undo_history(&self) -> Vec<String> {
        let Some(context) = &self.context.get() else {
            return Vec::new();
        };
        let store = Arc::clone(&self.recovery_store);
        let tx_id = context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.undo_history(&tx_id).unwrap_or_default())
            .await
            .unwrap_or_default()
    }

    pub fn recover_last_snapshot(&self) -> Option<TransactionSnapshot> {
        let Some(context) = &self.context.get() else {
            return None;
        };
        let store = Arc::clone(&self.recovery_store);
        let tx_id = context.tx_id().value().to_string();
        store.recover_latest(&tx_id.to_owned()).ok().flatten()
    }

    pub async fn cleanup(&self) {
        let Some(context) = &self.context.get() else {
            return;
        };
        let store = Arc::clone(&self.recovery_store);
        let tx_id = context.tx_id().value().to_string();
        let _ = tokio::task::spawn_blocking(move || store.cleanup(&tx_id)).await;
    }
}

//is still needed?
impl fmt::Debug for LocalRecoveryReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
           // .field("root_space", &self.root_space)
            .finish()
    }
}
