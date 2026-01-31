//! Transaction-scoped execution context shell for staging and transient pools.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use core_types::{HolonError, HolonId, TemporaryId};

use super::{TransactionContextHandle, TxId};
use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, Nursery, TransientCollection, TransientHolonManager, TransientManagerAccess,
};
use crate::dances::dance_initiator::DanceInitiator;
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    HolonsContextBehavior, TransientHolonBehavior,
};
use crate::{SmartReference, TransientReference};

/// Transaction-scoped execution context holding mutable transaction state.
#[derive(Debug)]
pub struct TransactionContext {
    tx_id: TxId,
    is_open: AtomicBool,
    space_manager: Arc<HolonSpaceManager>,
    nursery: Arc<Nursery>,
    transient_manager: Arc<TransientHolonManager>,
}

impl TransactionContext {
    /// Creates a new transaction context with its own staging and transient pools.
    pub(super) fn new(tx_id: TxId, space_manager: Arc<HolonSpaceManager>) -> Arc<Self> {
        Arc::new_cyclic(|weak_ctx| TransactionContext {
            tx_id,
            is_open: AtomicBool::new(true),
            space_manager,
            nursery: Arc::new(Nursery::new(tx_id, weak_ctx.clone())),
            transient_manager: Arc::new(TransientHolonManager::new_empty(tx_id, weak_ctx.clone())),
        })
    }

    /// Creates a handle to this transaction context for holon references.
    pub(crate) fn handle(self: &Arc<Self>) -> TransactionContextHandle {
        TransactionContextHandle::new(Arc::clone(self))
    }

    /// Returns the transaction id.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Returns whether the transaction is still open.
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Acquire)
    }

    /// Returns a strong reference to the space manager.
    pub fn space_manager(&self) -> Arc<HolonSpaceManager> {
        Arc::clone(&self.space_manager)
    }

    /// Provides access to the transaction-owned nursery.
    pub fn nursery(&self) -> Arc<Nursery> {
        Arc::clone(&self.nursery)
    }

    /// Provides access to the transaction-owned transient manager.
    pub fn transient_manager(&self) -> Arc<TransientHolonManager> {
        Arc::clone(&self.transient_manager)
    }

    // Public accessors for staging/transient behaviors (transaction-scoped).
    pub fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn HolonStagingBehavior + Send + Sync>
    }

    pub fn get_transient_behavior_service(&self) -> Arc<dyn TransientHolonBehavior + Send + Sync> {
        Arc::clone(&self.transient_manager) as Arc<dyn TransientHolonBehavior + Send + Sync>
    }

    // Internal privileged accessors for reference resolution.
    pub(crate) fn nursery_access_internal(&self) -> Arc<dyn NurseryAccessInternal + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn NurseryAccessInternal + Send + Sync>
    }

    pub(crate) fn transient_manager_access_internal(
        &self,
    ) -> Arc<dyn TransientManagerAccessInternal + Send + Sync> {
        Arc::clone(&self.transient_manager) as Arc<dyn TransientManagerAccessInternal + Send + Sync>
    }

    pub(crate) fn cache_access_internal(&self) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        self.require_space_manager().get_cache_access()
    }

    // Public import/export for session-state transport.
    pub fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.nursery.export_staged_holons()
    }

    pub fn import_staged_holons(
        &self,
        staged_holons: SerializableHolonPool,
    ) -> Result<(), HolonError> {
        let context = self
            .space_manager()
            .get_transaction_manager()
            .get_transaction(&self.tx_id())?
            .ok_or_else(|| HolonError::ServiceNotAvailable("TransactionContext".into()))?;

        let bound_pool = staged_holons.bind(context)?;
        self.nursery.import_staged_holons(bound_pool)
    }

    pub fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.transient_manager.export_transient_holons()
    }

    pub fn import_transient_holons(
        &self,
        transient_holons: SerializableHolonPool,
    ) -> Result<(), HolonError> {
        let context = self
            .space_manager()
            .get_transaction_manager()
            .get_transaction(&self.tx_id())?
            .ok_or_else(|| HolonError::ServiceNotAvailable("TransactionContext".into()))?;

        let bound_pool = transient_holons.bind(context)?;
        self.transient_manager.import_transient_holons(bound_pool)
    }

    fn require_space_manager(&self) -> Arc<HolonSpaceManager> {
        // Space manager lifetime is guaranteed by the strong Arc stored on the context.
        Arc::clone(&self.space_manager)
    }

    /// This function converts a TemporaryId into a validated TransientReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    pub fn transient_reference_for_id(
        self: &Arc<Self>,
        id: &TemporaryId,
    ) -> Result<TransientReference, HolonError> {
        // Validate id exists in this tx’s transient pool
        self.transient_manager().get_holon_by_id(id)?;
        Ok(TransientReference::from_temporary_id(self.handle(), id))
    }
}

impl HolonsContextBehavior for TransactionContext {
    fn tx_id(&self) -> TxId {
        self.tx_id
    }

    fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Acquire)
    }

    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        self.require_space_manager().get_cache_access()
    }

    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync> {
        self.require_space_manager().get_holon_service()
    }

    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError> {
        self.require_space_manager().get_dance_initiator()
    }

    // NOTE: This reacquires Arc<TransactionContext> via TransactionManager because HolonsContextBehavior
    // must remain object-safe for now. Once Phase 1.4 consolidates execution under TransactionContext,
    // this should become a simple `self: &Arc<Self>` method using `self.handle()`.
    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError> {
        let maybe_holon_id = self.require_space_manager().get_space_holon_id()?;
        let Some(holon_id) = maybe_holon_id else {
            return Ok(None);
        };

        // Reacquire the Arc<TransactionContext> so we can create a TransactionContextHandle.
        let context = self
            .require_space_manager()
            .get_transaction_manager()
            .get_transaction(&self.tx_id())?
            .ok_or_else(|| HolonError::ServiceNotAvailable("TransactionContext".into()))?;

        let transaction_handle = context.handle();

        Ok(Some(HolonReference::Smart(SmartReference::new_from_id(transaction_handle, holon_id))))
    }

    fn set_space_holon_id(&self, space_holon_id: HolonId) -> Result<(), HolonError> {
        self.require_space_manager().set_space_holon_id(space_holon_id)
    }

    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>> {
        self.require_space_manager().get_transient_state()
    }
}
