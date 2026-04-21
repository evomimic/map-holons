use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use core_types::HolonError;
use holons_client::{ClientSession, Receptor};
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::TransientReference;

pub struct RuntimeSession {
    space_manager: Arc<HolonSpaceManager>,
    recovery: Option<Arc<Receptor>>,
    active_sessions: RwLock<HashMap<TxId, Arc<ClientSession>>>,
    archived_sessions: RwLock<HashMap<TxId, Arc<ClientSession>>>,
}

impl RuntimeSession {
    pub fn new(
        space_manager: Arc<HolonSpaceManager>,
        recovery: Option<Arc<Receptor>>,
    ) -> Self {
        Self {
            space_manager,
            recovery,
            active_sessions: RwLock::new(HashMap::new()),
            archived_sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn restore_open_sessions(&self) -> Result<usize, HolonError> {
        let Some(recovery) = self.recovery.clone() else {
            return Ok(0);
        };

        let tx_ids = match recovery.as_ref() {
            Receptor::LocalRecovery(r) => r.list_open_sessions()?,
            _ => return Ok(0),
        };

        let mut restored = 0usize;
        let mut active = self.active_sessions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_sessions: {}",
                e
            ))
        })?;

        for tx_id in tx_ids {
            let session = Arc::new(ClientSession::recover(
                Arc::clone(&self.space_manager),
                Some(Arc::clone(&recovery)),
                tx_id,
            )?);

            active.insert(session.tx_id(), session);
            restored += 1;
        }

        Ok(restored)
    }

    pub async fn begin_transaction(&self) -> Result<TxId, HolonError> {
        let session = Arc::new(ClientSession::open_new(
            Arc::clone(&self.space_manager),
            self.recovery.clone(),
        )?);

        session.persist("begin_transaction", true).await?;

        let tx_id = session.tx_id();
        let mut active = self.active_sessions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_sessions: {}",
                e
            ))
        })?;
        active.insert(tx_id, session);

        Ok(tx_id)
    }

    /// Registers an already-opened recovered client session into the active pool.
    ///
    /// This is the seam startup recovery uses after reopening a transaction
    /// with its preserved `TxId`.
    pub fn register_recovered_session(
        &self,
        session: Arc<ClientSession>,
    ) -> Result<TxId, HolonError> {
        let tx_id = session.tx_id();

        let mut active = self.active_sessions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_sessions: {}",
                e
            ))
        })?;
        active.insert(tx_id, session);

        Ok(tx_id)
    }

    pub fn get_transaction(&self, tx_id: &TxId) -> Result<Arc<TransactionContext>, HolonError> {
        Ok(Arc::clone(self.get_client_session(tx_id)?.context()))
    }

    pub fn get_client_session(&self, tx_id: &TxId) -> Result<Arc<ClientSession>, HolonError> {
        {
            let active = self.active_sessions.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on active_sessions: {}",
                    e
                ))
            })?;
            if let Some(session) = active.get(tx_id) {
                return Ok(Arc::clone(session));
            }
        }

        let archived = self.archived_sessions.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on archived_sessions: {}",
                e
            ))
        })?;

        archived.get(tx_id).cloned().ok_or_else(|| {
            HolonError::InvalidParameter(format!(
                "No transaction found for tx_id={}",
                tx_id.value()
            ))
        })
    }

    pub async fn persist_success(
        &self,
        tx_id: &TxId,
        description: &str,
        disable_undo: bool,
    ) -> Result<(), HolonError> {
        if let Ok(session) = self.get_client_session(tx_id) {
            session.persist(description, disable_undo).await?;
        }
        Ok(())
    }

    pub async fn commit_transaction(
        &self,
        tx_id: &TxId,
    ) -> Result<TransientReference, HolonError> {
        let session = self.get_client_session(tx_id)?;
        let transient_ref = session.context().commit()?;

        session.cleanup().await?;
        self.archive_transaction(tx_id)?;

        Ok(transient_ref)
    }

    pub fn archive_transaction(&self, tx_id: &TxId) -> Result<(), HolonError> {
        let mut active = self.active_sessions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on active_sessions: {}",
                e
            ))
        })?;
        let mut archived = self.archived_sessions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on archived_sessions: {}",
                e
            ))
        })?;

        if let Some(session) = active.remove(tx_id) {
            archived.insert(*tx_id, session);
        }

        Ok(())
    }

    pub fn space_manager(&self) -> &Arc<HolonSpaceManager> {
        &self.space_manager
    }
}

impl std::fmt::Debug for RuntimeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let active_count = self.active_sessions.read().map(|g| g.len()).unwrap_or(0);
        let archived_count = self.archived_sessions.read().map(|g| g.len()).unwrap_or(0);

        f.debug_struct("RuntimeSession")
            .field("active_sessions", &active_count)
            .field("archived_sessions", &archived_count)
            .finish()
    }
}



#[cfg(test)]
mod tests {
    use std::any::Any;

    use core_types::{HolonError, HolonId, LocalId, RelationshipName};
    use holons_core::core_shared_objects::{
        Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
    };
    use holons_core::reference_layer::{
        HolonServiceApi, StagedReference, TransientReference,
    };

    use super::*;

    #[derive(Debug)]
    struct TestHolonService;

    fn unreachable_in_runtime_session_tests<T>() -> Result<T, HolonError> {
        Err(HolonError::NotImplemented(
            "TestHolonService".to_string(),
        ))
    }

    impl HolonServiceApi for TestHolonService {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _staged_references: &[StagedReference],
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_runtime_session_tests()
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_runtime_session_tests()
        }
    }

    fn build_test_space_manager() -> Arc<HolonSpaceManager> {
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ))
    }

    fn tx_id(value: u64) -> TxId {
        serde_json::from_value(serde_json::json!(value)).expect("tx_id should deserialize")
    }

    fn insert_recovered_transaction(
        session: &RuntimeSession,
        recovered_tx_id: TxId,
    ) -> Arc<ClientSession> {
        let recovered_session = Arc::new(
            ClientSession::recover(
                Arc::clone(session.space_manager()),
                None,
                recovered_tx_id.value().to_string(),
            )
            .expect("recovered session should open"),
        );

        session
            .register_recovered_session(Arc::clone(&recovered_session))
            .expect("recovered session should register");

        recovered_session
    }

    #[test]
    fn recovered_transaction_is_retrievable_from_active_pool() {
        let space_manager = build_test_space_manager();
        let session = RuntimeSession::new(space_manager, None);
        let recovered_tx_id = tx_id(41);

        let recovered_session = insert_recovered_transaction(&session, recovered_tx_id);

        let lookup = session
            .get_transaction(&recovered_tx_id)
            .expect("recovered transaction lookup should succeed");

        assert!(
            Arc::ptr_eq(recovered_session.context(), &lookup),
            "runtime session should return the recovered context stored for the tx"
        );
    }

    #[test]
    fn archived_recovered_transaction_remains_retrievable() {
        let space_manager = build_test_space_manager();
        let session = RuntimeSession::new(space_manager, None);
        let recovered_tx_id = tx_id(77);

        let recovered_session = insert_recovered_transaction(&session, recovered_tx_id);
        session
            .archive_transaction(&recovered_tx_id)
            .expect("archive should succeed");

        let lookup = session
            .get_transaction(&recovered_tx_id)
            .expect("archived recovered transaction lookup should succeed");

        assert!(
            Arc::ptr_eq(recovered_session.context(), &lookup),
            "archived recovered transaction should still resolve to the original context"
        );
    }

    #[tokio::test]
    async fn begin_transaction_after_recovered_context_uses_higher_tx_id() {
        let space_manager = build_test_space_manager();
        let session = RuntimeSession::new(space_manager,None);
        let recovered_tx_id = tx_id(120);

        insert_recovered_transaction(&session, recovered_tx_id);

        let new_tx_id = session
            .begin_transaction().await
            .expect("new transaction should open after recovery");

        assert!(
            new_tx_id.value() > recovered_tx_id.value(),
            "new tx_id should advance beyond the recovered tx_id to avoid collisions"
        );
    }
}
