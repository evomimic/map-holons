use std::sync::Arc;

use core_types::HolonError;
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager,
    transactions::{TransactionContext, TxId},
};

use crate::Receptor;

//#[derive(Debug)]
pub struct ClientSession {
    context: Arc<TransactionContext>,
    recovery: Option<Arc<Receptor>>,
}

impl ClientSession {
    /// Open a new session for a new transaction, optionally with a recovery receptor for state persistence.
    pub fn open_new(
        space_manager: Arc<HolonSpaceManager>,
        recovery: Option<Arc<Receptor>>,
    ) -> Result<Self, HolonError> {
        let context = space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&space_manager))?;

        Ok(Self { context, recovery })
    }

    /// Open a session for an existing transaction, restoring state from the recovery receptor if available.
    pub fn recover(
        space_manager: Arc<HolonSpaceManager>,
        recovery: Option<Arc<Receptor>>,
        tx_id: String,
    ) -> Result<Self, HolonError> {
        let tx_id = TxId::from_str(&tx_id)
            .ok_or_else(|| HolonError::InvalidParameter("invalid recovered tx_id".into()))?;

        let context = space_manager
            .get_transaction_manager()
            .open_transaction_with_id(Arc::clone(&space_manager), tx_id)?;

        let session = Self { context, recovery };
        session.restore_from_recovery()?;
        Ok(session)
    }

    pub fn tx_id(&self) -> TxId {
        self.context.tx_id()
    }

    pub fn context(&self) -> &Arc<TransactionContext> {
        &self.context
    }

    /// Restore transaction state from the recovery receptor, if available.
    fn restore_from_recovery(&self) -> Result<(), HolonError> {
        let Some(recovery) = self.recovery.as_ref() else {
            return Ok(());
        };

        match recovery.as_ref() {
            Receptor::LocalRecovery(r) => {
                if let Some(snapshot) = r.recover_latest(&self.tx_id().value().to_string())? {
                    snapshot.restore_into(&self.context)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Undo the last transaction command, if possible.
    pub async fn undo_last(&self) -> Result<(), HolonError> {
        let Some(recovery) = self.recovery.as_ref() else {
            return Ok(());
        };

        if let Receptor::LocalRecovery(r) = recovery.as_ref() {
            if let Some(tx_snapshot) = r.undo(&self.tx_id().value().to_string()).await? {
                tx_snapshot.restore_into(&self.context)?;
                return Ok(());
            } else {
                return Err(HolonError::Misc(format!(
                    "No undo snapshot available for tx_id={}",
                    &self.tx_id().value()
                )));
            }
        } else {
            Ok(())
        }
    }
    
    /// Redo the last undone transaction, if available.
    pub async fn redo_last(&self) -> Result<(), HolonError> {
        let Some(recovery) = self.recovery.as_ref() else {
            return Ok(());
        };

        if let Receptor::LocalRecovery(r) = recovery.as_ref() {
            if let Some(tx_snapshot) = r.redo(&self.tx_id().value().to_string()).await? {
                tx_snapshot.restore_into(&self.context)?;
                return Ok(());
            } else {
                return Err(HolonError::Misc(format!(
                    "No napshot available for tx_id={}",
                    &self.tx_id().value()
                )));
            }
        } else {
            Ok(())
        }
    }

    /// Persist the current transaction state with the given description and options.
    pub async fn persist(&self, description: &str, disable_undo: bool, snapshot_after: bool, marker_id: Option<String>, marker_label: Option<String>) -> Result<(), HolonError> {
        let Some(recovery) = self.recovery.as_ref() else {
            return Ok(());
        };

        if let Receptor::LocalRecovery(r) = recovery.as_ref() {
            return r.persist(&self.context, description, disable_undo, snapshot_after, marker_id, marker_label).await;
        } else {
            Ok(())
        }
    }

    /// Cleanup recovery state for this transaction, if applicable.
    pub async fn cleanup(&self) -> Result<(), HolonError> {
        let Some(recovery) = self.recovery.as_ref() else {
            return Ok(());
        };

        if let Receptor::LocalRecovery(r) = recovery.as_ref() {
            return r.cleanup(&self.tx_id().value().to_string()).await;
        } else {
            Ok(())
        }
    }
}
