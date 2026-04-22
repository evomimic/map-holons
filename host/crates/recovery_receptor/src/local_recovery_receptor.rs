use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use client_shared_types::base_receptor::{BaseReceptor, ReceptorType};
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;

use super::storage::transaction_snapshot::TransactionSnapshot;
use super::storage::{RecoveryStore, TransactionRecoveryStore};

pub struct LocalRecoveryReceptor {
    receptor_id: String,
    receptor_type: ReceptorType,
    properties: HashMap<String, String>,
    recovery_store: Arc<TransactionRecoveryStore>,
}

impl LocalRecoveryReceptor {
    pub fn new(base_receptor: BaseReceptor) -> Result<Self, HolonError> {
        let client_any = base_receptor
            .client_handler
            .as_ref()
            .expect("a handler is required for LocalRecoveryReceptor")
            .clone();

        let recovery_store = client_any.downcast::<TransactionRecoveryStore>().map_err(|_| {
            HolonError::DowncastFailure(format!(
                "Failed to cast client handler for LocalRecoveryReceptor '{}'",
                base_receptor.receptor_id
            ))
        })?;

        Ok(Self {
            receptor_id: base_receptor.receptor_id.clone(),
            receptor_type: base_receptor.receptor_type,
            properties: base_receptor.properties.clone(),
            recovery_store,
        })
    }

    pub fn list_open_sessions(&self) -> Result<Vec<String>, HolonError> {
        self.recovery_store.list_open_sessions()
    }

    pub fn recover_latest(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        self.recovery_store.recover_latest(tx_id)
    }

    pub async fn persist(
        &self,
        context: &Arc<TransactionContext>,
        description: &str,
        disable_undo: bool,
    ) -> Result<(), HolonError> {
        let store = Arc::clone(&self.recovery_store);
        let context = Arc::clone(context);
        let description = description.to_string();

        tokio::task::spawn_blocking(move || store.persist(&context, &description, disable_undo))
            .await
            .map_err(|e| HolonError::Misc(format!("persist join error: {e}")))?
    }

    pub async fn undo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let store = Arc::clone(&self.recovery_store);
        let tx_id = tx_id.to_string();

        tokio::task::spawn_blocking(move || store.undo(&tx_id))
            .await
            .map_err(|e| HolonError::Misc(format!("undo join error: {e}")))?
    }

    pub async fn redo(&self, tx_id: &str) -> Result<Option<TransactionSnapshot>, HolonError> {
        let store = Arc::clone(&self.recovery_store);
        let tx_id = tx_id.to_string();

        tokio::task::spawn_blocking(move || store.redo(&tx_id))
            .await
            .map_err(|e| HolonError::Misc(format!("redo join error: {e}")))?
    }

    pub fn can_undo(&self, tx_id: &str) -> Result<bool, HolonError> {
        self.recovery_store.can_undo(tx_id)
    }

    pub fn can_redo(&self, tx_id: &str) -> Result<bool, HolonError> {
        self.recovery_store.can_redo(tx_id)
    }

    pub async fn list_undo_history(&self, tx_id: &str) -> Result<Vec<String>, HolonError> {
        let store = Arc::clone(&self.recovery_store);
        let tx_id = tx_id.to_string();

        tokio::task::spawn_blocking(move || store.undo_history(&tx_id))
            .await
            .map_err(|e| HolonError::Misc(format!("undo_history join error: {e}")))?
    }

    pub async fn cleanup(&self, tx_id: &str) -> Result<(), HolonError> {
        let store = Arc::clone(&self.recovery_store);
        let tx_id = tx_id.to_string();

        tokio::task::spawn_blocking(move || store.cleanup(&tx_id))
            .await
            .map_err(|e| HolonError::Misc(format!("cleanup join error: {e}")))?
    }
}

impl fmt::Debug for LocalRecoveryReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalRecoveryReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}
