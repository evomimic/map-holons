//! Per-space transaction authority for creating and registering transactions.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use core_types::HolonError;

use crate::core_shared_objects::space_manager::HolonSpaceManager;

use super::tx_id::TransactionIdGenerator;
use super::{TransactionContext, TxId};

/// Owns transaction id generation and transaction registration for a space.
#[derive(Debug)]
pub struct TransactionManager {
    /// Monotonic id generator scoped to this space.
    id_generator: TransactionIdGenerator,
    /// Registry of open transactions keyed by id.
    transactions: RwLock<HashMap<TxId, Arc<TransactionContext>>>,
}

impl TransactionManager {
    /// Creates a new transaction manager with an empty registry.
    pub fn new() -> Self {
        // Initialize
        Self {
            id_generator: TransactionIdGenerator::new(),
            transactions: RwLock::new(HashMap::new()),
        }
    }

    /// Creates and registers the implicit default transaction for this space.
    pub fn open_default_transaction(
        &self,
        space_manager: &Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        // Delegate to the internal transaction opener.
        self.open_transaction(space_manager)
    }

    /// Looks up a transaction by id.
    pub fn get_transaction(
        &self,
        tx_id: &TxId,
    ) -> Result<Option<Arc<TransactionContext>>, HolonError> {
        // Acquire read access to the registry.
        let guard = self.transactions.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transactions: {}",
                e
            ))
        })?;
        // Clone the Arc for return.
        let transaction = guard.get(tx_id).map(Arc::clone);
        // Drop the lock before returning.
        drop(guard);
        Ok(transaction)
    }

    fn open_transaction(
        &self,
        space_manager: &Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        // Allocate a new transaction id.
        let tx_id = self.id_generator.next_id();
        // Build the transaction context with a weak space reference.
        let context = Arc::new(TransactionContext::new(tx_id, Arc::downgrade(space_manager)));

        // Register the transaction while holding the lock.
        let mut guard = self.transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on transactions: {}",
                e
            ))
        })?;
        guard.insert(tx_id, Arc::clone(&context));
        // Drop the lock before returning.
        drop(guard);

        Ok(context)
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::holon_pool::SerializableHolonPool;
    use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
    use crate::core_shared_objects::space_manager::HolonSpaceManager;
    use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
    use crate::core_shared_objects::{Holon, HolonCollection, Nursery, ServiceRoutingPolicy};
    use crate::core_shared_objects::{RelationshipMap, TransientHolonManager};
    use crate::reference_layer::{HolonServiceApi, HolonsContextBehavior, TransientReference};
    use core_types::{HolonError, HolonId, LocalId, RelationshipName};
    use std::any::Any;
    use std::collections::HashSet;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestHolonService;

    fn not_implemented<T>() -> Result<T, HolonError> {
        Err(HolonError::NotImplemented("TestHolonService".to_string()))
    }

    impl HolonServiceApi for TestHolonService {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &dyn HolonsContextBehavior,
        ) -> Result<TransientReference, HolonError> {
            not_implemented()
        }

        fn delete_holon_internal(&self, _local_id: &LocalId) -> Result<(), HolonError> {
            not_implemented()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &dyn HolonsContextBehavior,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            not_implemented()
        }

        fn fetch_holon_internal(&self, _id: &HolonId) -> Result<Holon, HolonError> {
            not_implemented()
        }

        fn fetch_related_holons_internal(
            &self,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            not_implemented()
        }

        fn get_all_holons_internal(
            &self,
            _context: &dyn HolonsContextBehavior,
        ) -> Result<HolonCollection, HolonError> {
            not_implemented()
        }

        fn load_holons_internal(
            &self,
            _ctx: &dyn HolonsContextBehavior,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            not_implemented()
        }
    }

    fn build_space_manager() -> Arc<HolonSpaceManager> {
        // Step 1: Create the minimal Holon service stub.
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        // Step 2: Construct an empty nursery and transient manager.
        let nursery = Nursery::new();
        let transient_manager = TransientHolonManager::new_empty();
        // Step 3: Build the space manager with a restrictive cache policy.
        Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
            nursery,
            transient_manager,
        ))
    }

    #[test]
    fn open_default_transaction_creates_and_registers() {
        // Step 1: Create a space manager and transaction manager.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();
        // Step 2: Open the default transaction.
        let transaction =
            tm.open_default_transaction(&space_manager).expect("default transaction should open");
        // Step 3: Look up the transaction by id.
        let lookup = tm
            .get_transaction(&transaction.tx_id())
            .expect("transaction lookup should succeed")
            .expect("transaction should be registered");
        // Step 4: Verify identity and id stability.
        assert!(Arc::ptr_eq(&transaction, &lookup));
        assert_eq!(transaction.tx_id(), lookup.tx_id());
    }

    #[test]
    fn tx_id_is_unique_and_monotonic() {
        // Step 1: Open multiple transactions through the internal helper.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();
        let mut ids = Vec::new();
        for _ in 0..5 {
            let transaction = tm.open_transaction(&space_manager).expect("transaction should open");
            ids.push(transaction.tx_id());
        }
        // Step 2: Ensure ids are unique.
        let mut unique = HashSet::new();
        for id in &ids {
            assert!(unique.insert(*id));
        }
        // Step 3: Ensure ids are monotonically increasing.
        for window in ids.windows(2) {
            assert!(window[0].value() < window[1].value());
        }
    }

    #[test]
    fn transaction_context_owns_pools() {
        // Step 1: Open a default transaction.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();
        let transaction =
            tm.open_default_transaction(&space_manager).expect("default transaction should open");
        // Step 2: Export staged holons from the transaction nursery.
        let staged =
            transaction.nursery().export_staged_holons().expect("staged export should succeed");
        // Step 3: Export transient holons from the transaction manager.
        let transient = transaction
            .transient_manager()
            .export_transient_holons()
            .expect("transient export should succeed");
        assert_eq!(staged, SerializableHolonPool::default());
        assert_eq!(transient, SerializableHolonPool::default());
    }

    #[test]
    fn transaction_context_weak_space_manager_upgrade() {
        // Step 1: Create a space manager and open a transaction.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();
        let transaction =
            tm.open_default_transaction(&space_manager).expect("default transaction should open");
        // Step 2: Weak upgrade succeeds while the space manager is alive.
        assert!(transaction.space_manager().is_ok());
        // Step 3: Drop the last strong ref and confirm upgrade failure.
        drop(space_manager);
        assert!(transaction.space_manager().is_err());
    }
}
