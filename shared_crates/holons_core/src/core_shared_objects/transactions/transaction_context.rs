//! Transaction-scoped execution context shell for staging and transient pools.

use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, RwLock,
};

use core_types::{HolonError, HolonId, TemporaryId};

use super::{HostCommitExecutionGuard, TransactionContextHandle, TransactionLifecycleState, TxId};
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, HolonPool, Nursery, TransientCollection, TransientHolonManager,
    TransientManagerAccess,
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
    lifecycle_state: AtomicU8,
    /// Host-ingress concurrency guard only:
    /// prevents external request mutations from racing in-flight commit ingress.
    host_commit_in_progress: AtomicBool,
    space_manager: Arc<HolonSpaceManager>,
    nursery: Arc<Nursery>,
    transient_manager: Arc<TransientHolonManager>,
}

impl TransactionContext {
    /// Creates a new transaction context with its own staging and transient pools.
    pub(super) fn new(tx_id: TxId, space_manager: Arc<HolonSpaceManager>) -> Arc<Self> {
        Arc::new_cyclic(|weak_ctx| TransactionContext {
            tx_id,
            lifecycle_state: AtomicU8::new(TransactionLifecycleState::Open.as_u8()),
            host_commit_in_progress: AtomicBool::new(false),
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

    /// Returns the current lifecycle state for this transaction.
    pub fn lifecycle_state(&self) -> TransactionLifecycleState {
        TransactionLifecycleState::from_u8(self.lifecycle_state.load(Ordering::Acquire))
    }

    /// Returns whether the transaction is still open.
    pub fn is_open(&self) -> bool {
        self.lifecycle_state() == TransactionLifecycleState::Open
    }

    /// Enforces host-side external mutation constraints.
    ///
    /// External write/mutation entrypoints are only valid while the transaction is
    /// `Open` and no host commit ingress is currently in progress.
    ///
    /// This includes host-side transient creation requests (for example
    /// `create_new_holon`) in addition to staging/property/relationship mutations.
    /// Read/query entrypoints are governed separately and are not blocked here.
    pub fn ensure_open_for_external_mutation(&self) -> Result<(), HolonError> {
        if self.lifecycle_state() != TransactionLifecycleState::Open {
            return Err(HolonError::TransactionNotOpen {
                tx_id: self.tx_id.value(),
                state: format!("{:?}", self.lifecycle_state()),
            });
        }

        if self.is_host_commit_in_progress() {
            return Err(HolonError::TransactionCommitInProgress { tx_id: self.tx_id.value() });
        }

        Ok(())
    }

    /// Enforces host-side commit-entry constraints.
    ///
    /// This guard is used for external commit-like ingress requests (`commit`,
    /// `load_holons`) and does not restrict read/query entrypoints.
    pub fn ensure_commit_allowed(&self) -> Result<(), HolonError> {
        if self.lifecycle_state() == TransactionLifecycleState::Committed {
            return Err(HolonError::TransactionAlreadyCommitted { tx_id: self.tx_id.value() });
        }

        if self.is_host_commit_in_progress() {
            return Err(HolonError::TransactionCommitInProgress { tx_id: self.tx_id.value() });
        }

        Ok(())
    }

    /// Returns whether host ingress currently holds the commit guard for this transaction.
    pub fn is_host_commit_in_progress(&self) -> bool {
        self.host_commit_in_progress.load(Ordering::Acquire)
    }

    /// Attempts to begin host-side commit ingress.
    ///
    /// Returns `true` only when the guard is acquired by this caller.
    pub fn try_begin_host_commit_ingress(&self) -> bool {
        self.host_commit_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Ends host-side commit ingress and releases the guard.
    pub fn end_host_commit_ingress(&self) {
        self.host_commit_in_progress.store(false, Ordering::Release);
    }

    /// Acquires a scoped host-side commit ingress guard.
    ///
    /// Host-ingress concurrency guard only:
    /// Prevents external mutation requests from racing an in-flight commit.
    /// Not used by guest commit execution logic.
    pub fn begin_host_commit_ingress_guard(&self) -> Result<HostCommitExecutionGuard<'_>, HolonError> {
        HostCommitExecutionGuard::acquire(self)
    }

    /// Transitions the transaction lifecycle from `Open` to `Committed`.
    ///
    /// Returns `true` only when the state transition is applied by this caller.
    pub fn try_transition_to_committed(&self) -> bool {
        self.lifecycle_state
            .compare_exchange(
                TransactionLifecycleState::Open.as_u8(),
                TransactionLifecycleState::Committed.as_u8(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Applies the `Open -> Committed` lifecycle transition.
    pub fn transition_to_committed(&self) -> Result<(), HolonError> {
        if self.try_transition_to_committed() {
            return Ok(());
        }

        let current_state = self.lifecycle_state();
        if current_state == TransactionLifecycleState::Committed {
            return Err(HolonError::TransactionAlreadyCommitted { tx_id: self.tx_id.value() });
        }

        Err(HolonError::InvalidTransactionTransition {
            tx_id: self.tx_id.value(),
            from_state: format!("{:?}", current_state),
            to_state: format!("{:?}", TransactionLifecycleState::Committed),
        })
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

    // Public import/export for session_state-state transport.
    pub fn export_staged_holons(&self) -> Result<HolonPool, HolonError> {
        self.nursery.export_staged_holons()
    }

    pub fn import_staged_holons(&self, staged_holons: HolonPool) -> Result<(), HolonError> {
        self.nursery.import_staged_holons(staged_holons)
    }

    pub fn export_transient_holons(&self) -> Result<HolonPool, HolonError> {
        self.transient_manager.export_transient_holons()
    }

    pub fn import_transient_holons(&self, transient_holons: HolonPool) -> Result<(), HolonError> {
        self.transient_manager.import_transient_holons(transient_holons)
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
        self.is_open()
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
