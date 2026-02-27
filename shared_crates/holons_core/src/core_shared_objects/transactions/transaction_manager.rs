//! Per-space transaction authority for creating and registering transactions.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use core_types::HolonError;

use crate::core_shared_objects::space_manager::HolonSpaceManager;

use super::tx_id::TransactionIdGenerator;
use super::{TransactionContext, TxId};

/// Owns transaction id generation and transaction registration for a space.
///
/// Ownership model:
/// - TransactionManager does NOT own transaction lifetimes.
/// - The registry stores only Weak<TransactionContext>.
/// - Callers own transactions by holding Arc<TransactionContext>.
#[derive(Debug)]
pub struct TransactionManager {
    /// Monotonic id generator scoped to this space.
    id_generator: TransactionIdGenerator,
    /// Registry of open transactions keyed by id (weak refs only).
    transactions: RwLock<HashMap<TxId, Weak<TransactionContext>>>,
}

impl TransactionManager {
    /// Creates a new transaction manager with an empty registry.
    pub fn new() -> Self {
        Self {
            id_generator: TransactionIdGenerator::new(),
            transactions: RwLock::new(HashMap::new()),
        }
    }

    /// Creates and registers the implicit default transaction for this space.
    pub fn open_default_transaction(
        &self,
        space_manager: Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        self.open_transaction(space_manager)
    }

    /// Creates and registers a transaction with a specific id.
    ///
    /// This is intended for IPC round-trips where the originating side
    /// supplies an explicit tx_id that must be preserved.
    pub fn open_transaction_with_id(
        &self,
        space_manager: Arc<HolonSpaceManager>,
        tx_id: TxId,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        // Prevent collisions with an existing live transaction.
        if let Some(existing) = self.get_transaction(&tx_id)? {
            return Err(HolonError::DuplicateError(
                "Transaction".to_string(),
                format!("tx_id={}", existing.tx_id().value()),
            ));
        }

        // Ensure the generator will not re-issue this id.
        self.id_generator.bump_to_at_least(tx_id);

        // Build the transaction context with a STRONG space reference.
        let context = TransactionContext::new(tx_id, space_manager);

        // Register the transaction (weak only) while holding the lock briefly.
        let mut guard = self.transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on transactions: {}",
                e
            ))
        })?;
        guard.insert(tx_id, Arc::downgrade(&context));
        drop(guard);

        Ok(context)
    }

    /// Looks up a transaction by id.
    ///
    /// Returns:
    /// - Ok(Some(tx)) if the transaction is still alive.
    /// - Ok(None) if it is not present OR if the weak reference can no longer be upgraded
    ///   (meaning no one is holding the transaction alive).
    pub fn get_transaction(
        &self,
        tx_id: &TxId,
    ) -> Result<Option<Arc<TransactionContext>>, HolonError> {
        // Read lock: grab a clone of the Weak so we can drop the lock before upgrading.
        let weak = {
            let guard = self.transactions.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on transactions: {}",
                    e
                ))
            })?;
            guard.get(tx_id).cloned()
        };

        let Some(weak) = weak else {
            return Ok(None);
        };

        // Upgrade outside of the lock.
        let upgraded = weak.upgrade();

        // Optional: if upgrade fails, prune the dead entry.
        if upgraded.is_none() {
            let mut guard = self.transactions.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on transactions: {}",
                    e
                ))
            })?;
            // Only remove if the entry still matches the same (dead) weak.
            // (Best-effort cleanup; race-safe.)
            if let Some(current) = guard.get(tx_id) {
                if current.strong_count() == 0 {
                    guard.remove(tx_id);
                }
            }
        }

        Ok(upgraded)
    }

    fn open_transaction(
        &self,
        space_manager: Arc<HolonSpaceManager>,
    ) -> Result<Arc<TransactionContext>, HolonError> {
        // Allocate a new transaction id.
        let tx_id = self.id_generator.next_id();

        // Build the transaction context with a STRONG space reference.
        let context = TransactionContext::new(tx_id, space_manager);

        // Register the transaction (weak only) while holding the lock briefly.
        let mut guard = self.transactions.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on transactions: {}",
                e
            ))
        })?;
        guard.insert(tx_id, Arc::downgrade(&context));
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
    use crate::core_shared_objects::{
        Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
    };
    use crate::reference_layer::{HolonServiceApi, TransientReference};
    use core_types::{HolonError, HolonId, LocalId, RelationshipName};
    use std::any::Any;
    use std::collections::HashSet;

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
            _context: &Arc<TransactionContext>,
        ) -> Result<TransientReference, HolonError> {
            not_implemented()
        }

        fn delete_holon_internal(&self, _local_id: &LocalId) -> Result<(), HolonError> {
            not_implemented()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            not_implemented()
        }

        fn fetch_holon_internal(
            &self,
            context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            not_implemented()
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            not_implemented()
        }

        fn get_all_holons_internal(
            &self,
            context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            not_implemented()
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            not_implemented()
        }
    }

    fn build_space_manager() -> Arc<HolonSpaceManager> {
        // Step 1: Create the minimal Holon service stub.
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        // Step 2: Build the space manager with a restrictive cache policy.
        Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ))
    }

    #[test]
    fn open_default_transaction_creates_and_registers() {
        // Step 1: Create a space manager and transaction manager.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();

        // Step 2: Open the default transaction.
        let transaction = tm
            .open_default_transaction(Arc::clone(&space_manager))
            .expect("default transaction should open");

        // Step 3: Look up the transaction by id.
        let lookup = tm
            .get_transaction(&transaction.tx_id())
            .expect("transaction lookup should succeed")
            .expect("transaction should be registered and alive");

        // Step 4: Verify identity and id stability.
        assert!(Arc::ptr_eq(&transaction, &lookup));
        assert_eq!(transaction.tx_id(), lookup.tx_id());
    }

    #[test]
    fn tx_id_is_unique_and_monotonic() {
        // Step 1: Open multiple transactions.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();

        let mut ids = Vec::new();
        for _ in 0..5 {
            let transaction =
                tm.open_transaction(Arc::clone(&space_manager)).expect("transaction should open");
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
        let transaction = tm
            .open_default_transaction(Arc::clone(&space_manager))
            .expect("default transaction should open");

        // Step 2: Export staged holons from the transaction nursery.
        let staged = transaction.export_staged_holons().expect("staged export should succeed");

        // Step 3: Export transient holons from the transaction manager.
        let transient =
            transaction.export_transient_holons().expect("transient export should succeed");

        assert_eq!(staged.len(), 0);
        assert!(staged.holons_by_id().is_empty());
        assert!(staged.keyed_index().is_empty());

        assert_eq!(transient.len(), 0);
        assert!(transient.holons_by_id().is_empty());
        assert!(transient.keyed_index().is_empty());
    }

    #[test]
    fn transaction_manager_lookup_returns_none_after_last_arc_dropped() {
        // Step 1: Create a space manager and open a transaction.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();

        let tx_id = {
            let transaction = tm
                .open_default_transaction(Arc::clone(&space_manager))
                .expect("default transaction should open");
            transaction.tx_id()
        }; // transaction Arc dropped here (no other strong owners in this test)

        // Step 2: Lookup should return None once the last Arc is dropped.
        let lookup = tm.get_transaction(&tx_id).expect("transaction lookup should succeed");
        assert!(lookup.is_none());
    }

    #[test]
    fn transaction_context_strong_space_manager_keeps_space_alive() {
        // Step 1: Create a space manager and open a transaction.
        let space_manager = build_space_manager();
        let tm = space_manager.get_transaction_manager();

        let transaction = tm
            .open_default_transaction(Arc::clone(&space_manager))
            .expect("default transaction should open");

        // Step 2: Drop the original Arc; TC should still keep the space alive.
        drop(space_manager);

        // Step 3: Accessing the space manager through TC should still work.
        let _ = transaction.space_manager();
    }
}
