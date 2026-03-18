use crate::client_shared_objects::ClientHolonService;

use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::ServiceRoutingPolicy;

use holons_core::dances::DanceInitiator;
use holons_core::reference_layer::HolonServiceApi;
use holons_recovery::transaction_snapshot::TransactionSnapshot;
use holons_recovery::RecoveryStore;

use std::sync::Arc;

/// Host-side session. Store presence implies autosave — no separate flag needed.
pub struct ClientSession {
    pub context: Arc<TransactionContext>,
    /// `None` = no recovery for this receptor. `Some` = persist after every command.
    pub recovery_store: Option<Arc<dyn RecoveryStore>>,
}

impl ClientSession {
    /// Called after every successful command.
    /// `description` is the command name — used in undo history display.
    /// `disable_undo` = true for bulk/loader ops that shouldn't be individually undoable.
    pub async fn persist(&self, description: &str, disable_undo: bool) {
        let Some(store) = &self.recovery_store else { return };
        let store = Arc::clone(store);
        let context = Arc::clone(&self.context);
        let description = description.to_string();

        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = store.persist(&context, &description, disable_undo) {
                tracing::warn!("[CLIENT SESSION] Persist failed: {e}");
            }
        })
        .await;
    }

    pub async fn undo(&self) -> Option<TransactionSnapshot> {
        let Some(store) = &self.recovery_store else { return None };
        let store = Arc::clone(store);
        let tx_id = self.context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.undo(&tx_id).ok().flatten())
            .await
            .ok()
            .flatten()
    }

    pub async fn redo(&self) -> Option<TransactionSnapshot> {
        let Some(store) = &self.recovery_store else { return None };
        let store = Arc::clone(store);
        let tx_id = self.context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.redo(&tx_id).ok().flatten())
            .await
            .ok()
            .flatten()
    }

    pub async fn list_undo_history(&self) -> Vec<String> {
        let Some(store) = &self.recovery_store else { return vec![] };
        let store = Arc::clone(store);
        let tx_id = self.context.tx_id().value().to_string();
        tokio::task::spawn_blocking(move || store.undo_history(&tx_id).unwrap_or_default())
            .await
            .unwrap_or_default()
    }

    pub fn recover_last_snapshot(&self) -> Option<TransactionSnapshot> {
        let Some(store) = &self.recovery_store else { return None };
        let store = Arc::clone(store);
        let tx_id = self.context.tx_id().value().to_string();
        store.recover_latest(&tx_id.to_owned()).ok().flatten()
    }

    pub async fn cleanup(&self) {
        let Some(store) = &self.recovery_store else { return };
        let store = Arc::clone(store);
        let tx_id = self.context.tx_id().value().to_string();
        let _ = tokio::task::spawn_blocking(move || store.cleanup(&tx_id)).await;
    }
}


/// Initializes a new client-side context with a fresh `HolonSpaceManager` and
/// an implicit default transaction.
///
/// This function sets up:
/// - A space manager configured with client-specific routing policies.
/// - An implicit transaction opened via the per-space `TransactionManager`.
/// - Injects the optional `DanceInitiator` for conductor calls.
/// - Injects the optional `RecoveryStore` for recovery operations.
///
/// # Returns
/// * A `ClientSession` backed by a `TransactionContext` and optional `RecoveryStore`.
pub fn init_client_context(
    initiator: Option<Arc<dyn DanceInitiator>>,
    recovery_store: Option<Arc<dyn RecoveryStore>>
) -> ClientSession {    
    
    let space_manager = init_client_runtime(initiator);

    // Open the default transaction for this space.
    // TransactionContext becomes the sole execution root and owns the space.
   let context = space_manager
        .get_transaction_manager()
        .open_new_transaction(Arc::clone(&space_manager))
        .expect("failed to open default client transaction");

    ClientSession { context, recovery_store }
}

/// Initializes a new client-side `HolonSpaceManager` without opening a transaction.
///
/// Same construction as `init_client_context()` but returns the space manager
/// directly, leaving transaction lifecycle to the caller (e.g., `RuntimeSession`).
pub fn init_client_runtime(initiator: Option<Arc<dyn DanceInitiator>>) -> Arc<HolonSpaceManager> {
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);

    Arc::new(HolonSpaceManager::new_with_managers(
        initiator,
        holon_service,
        None,
        ServiceRoutingPolicy::Combined,
    ))
}
