//! Transaction-scoped execution context shell for staging and transient pools.

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
};

use base_types::BaseValue;
use core_types::{HolonError, HolonId};
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::reference_layer::ReadableHolon;
use type_names::CorePropertyTypeName;

use super::{
    DanceInitiator, DanceRequest, DanceResponse, Holon, HolonCacheAccess,
    HolonCloneModel, HolonPool, HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonSpaceManager,
    HolonStagingBehavior, HostCommitExecutionGuard, LookupFacade, MutationFacade, Nursery,
    NurseryAccess,
    NurseryAccessInternal, SmartReference, TransactionContextHandle, TransactionLifecycleState,
    TransientHolonBehavior, TransientHolonManager, TransientManagerAccess,
    TransientReference, TxId,
};

/// Transaction-scoped operations used for lifecycle/access policy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransactionOperation {
    /// Create a new transient holon.
    CreateTransient,
    /// Stage/delete/load and other state mutation operations.
    MutateState,
    /// Commit execution within an already-open transaction.
    CommitExecution,
    /// Host-side external mutation ingress entry.
    HostMutationEntry,
}

/// Transaction-scoped execution context holding mutable transaction state.
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

impl fmt::Debug for TransactionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionContext")
            .field("tx_id", &self.tx_id)
            .field("is_open", &self.is_open())
            .finish()
    }
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

    // ---------------------------------------------------------------------
    // Execution Identity & Lifecycle
    // ---------------------------------------------------------------------

    /// Creates a handle to this transaction context for holon references.
    pub fn context_handle(self: &Arc<Self>) -> TransactionContextHandle {
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

    /// Transitions the transaction lifecycle from `Open` to `Committed`.
    ///
    /// Returns `true` only when the state transition is applied by this caller.
    fn try_transition_to_committed(&self) -> bool {
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
    fn transition_to_committed(&self) -> Result<(), HolonError> {
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

    /// Internal operation policy gate.
    ///
    /// This is the authoritative mutation/commit ingress policy matrix.
    /// All transaction-scoped mutation and commit ingress paths should enforce
    /// lifecycle/access policy through this method rather than per-call-site
    /// omission rules.
    ///
    /// Lookup/read-only execution paths are intentionally excluded from this gate.
    ///
    /// ## Lifecycle/Operation Matrix
    ///
    /// `host_commit_in_progress` only affects host-ingress mutation admission.
    /// It does not block commit execution itself.
    ///
    /// | Operation | `Open` + no host commit ingress | `Open` + host commit ingress | `Committed` |
    /// | --- | --- | --- | --- |
    /// | `CreateTransient` | Allowed | Allowed | Allowed |
    /// | `MutateState` | Allowed | Rejected (`TransactionCommitInProgress`) | Rejected (`TransactionAlreadyCommitted`) |
    /// | `HostMutationEntry` | Allowed | Rejected (`TransactionCommitInProgress`) | Rejected (`TransactionAlreadyCommitted`) |
    /// | `CommitExecution` | Allowed | Allowed | Rejected (`TransactionAlreadyCommitted`) |
    ///
    /// Any unknown/raw lifecycle value is rejected as `TransactionNotOpen`.
    pub(super) fn assert_allowed(&self, operation: TransactionOperation) -> Result<(), HolonError> {
        let raw_state = self.lifecycle_state.load(Ordering::Acquire);

        match operation {
            TransactionOperation::CreateTransient => {
                if raw_state == TransactionLifecycleState::Open.as_u8()
                    || raw_state == TransactionLifecycleState::Committed.as_u8()
                {
                    return Ok(());
                }
                Err(HolonError::TransactionNotOpen {
                    tx_id: self.tx_id.value(),
                    state: format!("Unknown({raw_state})"),
                })
            }
            TransactionOperation::MutateState | TransactionOperation::HostMutationEntry => {
                if raw_state != TransactionLifecycleState::Open.as_u8() {
                    if raw_state == TransactionLifecycleState::Committed.as_u8() {
                        return Err(HolonError::TransactionAlreadyCommitted {
                            tx_id: self.tx_id.value(),
                        });
                    }
                    return Err(HolonError::TransactionNotOpen {
                        tx_id: self.tx_id.value(),
                        state: format!("Unknown({raw_state})"),
                    });
                }

                if self.is_host_commit_in_progress() {
                    return Err(HolonError::TransactionCommitInProgress {
                        tx_id: self.tx_id.value(),
                    });
                }

                Ok(())
            }
            TransactionOperation::CommitExecution => {
                if raw_state == TransactionLifecycleState::Open.as_u8() {
                    return Ok(());
                }
                if raw_state == TransactionLifecycleState::Committed.as_u8() {
                    return Err(HolonError::TransactionAlreadyCommitted {
                        tx_id: self.tx_id.value(),
                    });
                }
                Err(HolonError::TransactionNotOpen {
                    tx_id: self.tx_id.value(),
                    state: format!("Unknown({raw_state})"),
                })
            }
        }
    }

    /// Commits the state of all staged holons and their relationships to the DHT.
    ///
    /// This function attempts to persist the state of all staged holons and their relationships
    /// to the distributed hash table (DHT). It can be called from both the client-side and the
    /// guest-side:
    /// - **On the client-side**: The call is delegated to the guest-side for execution, where the
    ///   actual DHT operations are performed.
    /// - **On the guest-side**: The commit process interacts directly with the DHT.
    ///
    /// The function returns either a `HolonError` (indicating a system-level failure) or a
    /// `CommitResponse`. If a `CommitResponse` is returned, it will indicate whether the commit
    /// was fully successful (`Complete`) or partially successful (`Incomplete`).
    ///
    /// # Commit Outcomes
    ///
    /// ## Complete Commit
    /// If the commit process fully succeeds:
    /// - The `CommitResponse` will have a `Complete` status.
    /// - All staged holons and their relationships are successfully persisted to the DHT.
    /// - The `CommitResponse` includes a list of all successfully saved holons, with their `record`
    /// (including their `LocalId`) populated.
    /// - The `space_manager`'s list of staged holons is cleared.
    ///
    /// ## Partial Commit
    /// If the commit process partially succeeds:
    /// - The `CommitResponse` will have an `Incomplete` status.
    /// - **No staged holons are removed** from the `space_manager`.
    /// - Holons that were successfully committed:
    ///     - Have their state updated to `Saved`.
    ///     - Include their saved node (indicating they were persisted).
    ///     - Are added to the `CommitResponse`'s `records` list.
    /// - Holons that were **not successfully committed**:
    ///     - Retain their previous state (unchanged).
    ///     - Have their `errors` vector populated with the errors encountered during the commit.
    ///     - Do **not** include a saved node.
    ///     - Are **not** added to the `CommitResponse`'s `records` list.
    /// - Correctable errors in the `errors` vector allow the `commit` call to be retried until the
    ///   process succeeds completely.
    ///
    /// ## Failure
    /// If the commit process fails entirely due to a system-level issue:
    /// - The function returns a `HolonError`.
    /// - No changes are made to the staged holons.
    ///
    /// # Arguments
    /// - `context`: The context to retrieve holon services.
    ///
    /// # Returns
    /// - `Ok(CommitResponse)`:
    ///     - If the commit process is successful (either completely or partially).
    ///     - Use the `CommitResponse`'s status to determine whether the commit is `Complete` or `Incomplete`.
    /// - `Err(HolonError)`:
    ///     - If a system-level failure prevents the commit process from proceeding.
    ///
    /// # Errors
    /// - Returns a `HolonError` if the commit operation encounters a system-level issue.
    ///
    pub fn commit(self: &Arc<Self>) -> Result<TransientReference, HolonError> {
        self.assert_allowed(TransactionOperation::CommitExecution)?;
        let staged_references = self.nursery.get_staged_references()?;
        let commit_response = self.get_holon_service().commit_internal(self, &staged_references)?;
        if self.should_transition_from_commit_response(&commit_response)? {
            self.nursery.clear_stage()?;
            self.transition_to_committed()?;
        }

        Ok(commit_response)
    }

    /// Loads holons from a loader bundle and applies terminal lifecycle semantics.
    ///
    /// This operation is commit-like by design: when the returned load response indicates
    /// `LoadCommitStatus = Complete`, this transaction transitions to `Committed`.
    pub fn load_holons_and_commit(
        self: &Arc<Self>,
        bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        self.assert_allowed(TransactionOperation::CommitExecution)?;
        let load_response = self.get_holon_service().load_holons_internal(self, bundle)?;
        if self.should_transition_from_load_response(&load_response)? {
            self.transition_to_committed_if_needed()?;
        }
        Ok(load_response)
    }

    pub(crate) fn fetch_holon_internal(
        self: &Arc<Self>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.get_holon_service().fetch_holon_internal(self, id)
    }

    pub(crate) fn new_transient_from_clone_model(
        &self,
        holon_clone_model: HolonCloneModel,
    ) -> Result<TransientReference, HolonError> {
        let transient_service =
            Arc::clone(&self.transient_manager) as Arc<dyn TransientHolonBehavior + Send + Sync>;
        transient_service.new_from_clone_model(holon_clone_model)
    }

    pub fn ensure_local_holon_space(self: &Arc<Self>) -> Result<HolonReference, HolonError> {
        self.get_holon_service().ensure_local_holon_space_internal(self)
    }

    // ---------------------------------------------------------------------
    // Host Commit Ingress Guard
    // ---------------------------------------------------------------------

    /// Enforces host-side external mutation constraints.
    ///
    /// External write/mutation entrypoints are only valid while the transaction is
    /// `Open` and no host commit ingress is currently in progress.
    ///
    /// Read/query entrypoints are governed separately and are not blocked here.
    /// Returns whether host ingress currently holds the commit guard for this transaction.
    fn is_host_commit_in_progress(&self) -> bool {
        self.host_commit_in_progress.load(Ordering::Acquire)
    }

    /// Attempts to begin host-side commit ingress.
    ///
    /// Returns `true` only when the guard is acquired by this caller.
    pub(super) fn try_begin_host_commit_ingress(&self) -> bool {
        self.host_commit_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Ends host-side commit ingress and releases the guard.
    pub(super) fn end_host_commit_ingress(&self) {
        self.host_commit_in_progress.store(false, Ordering::Release);
    }

    /// Acquires a scoped host-side commit ingress guard.
    ///
    /// Host-ingress concurrency guard only:
    /// Prevents external mutation requests from racing an in-flight commit.
    /// Not used by guest commit execution logic.
    pub fn begin_host_commit_ingress_guard(
        &self,
    ) -> Result<HostCommitExecutionGuard<'_>, HolonError> {
        let guard = HostCommitExecutionGuard::acquire(self)?;
        self.assert_allowed(TransactionOperation::CommitExecution)?;
        Ok(guard)
    }

    /// Fail-fast admission check for host-side external mutation ingress.
    ///
    /// This is intended for ingress routers/dispatchers to validate mutation
    /// policy before any request-building path that may perform side effects.
    pub fn ensure_host_mutation_entry_allowed(&self) -> Result<(), HolonError> {
        self.assert_allowed(TransactionOperation::HostMutationEntry)
    }

    // ---------------------------------------------------------------------
    // Public Execution Facades
    // ---------------------------------------------------------------------
    // These facades are the only public entrypoints for mutation and lookup
    // operations. They do not represent alternate execution surfaces —
    // they are thin, Arc-backed views over this TransactionContext.

    /// Returns a facade grouping all state-mutating operations.
    ///
    /// This method clones the underlying `Arc<TransactionContext>`.
    ///
    /// For repeated use, prefer capturing the facade once:
    ///
    /// ```ignore
    /// let mutation = ctx.mutation();
    /// for item in items {
    ///     mutation.stage_new_holon(item)?;
    /// }
    /// ```
    ///
    /// Instead of:
    ///
    /// ```ignore
    /// for item in items {
    ///     ctx.mutation().stage_new_holon(item)?; // Avoid repeated Arc::clone
    /// }
    /// ```
    ///
    /// Cloning `Arc` is inexpensive, but avoiding repeated clones in tight
    /// loops improves clarity and avoids unnecessary churn.
    pub fn mutation(self: &Arc<Self>) -> MutationFacade {
        MutationFacade {
            context: Arc::clone(self),
            holon_service: self.get_holon_service(),
            staging_service: self.get_staging_service(),
            transient_service: Arc::clone(&self.transient_manager)
                as Arc<dyn TransientHolonBehavior + Send + Sync>,
        }
    }

    /// Returns a facade grouping all indexed lookup operations.
    pub fn lookup(self: &Arc<Self>) -> LookupFacade {
        LookupFacade {
            context: Arc::clone(self),
            holon_service: self.get_holon_service(),
            staging_service: self.get_staging_service(),
            transient_service: Arc::clone(&self.transient_manager)
                as Arc<dyn TransientHolonBehavior + Send + Sync>,
        }
    }

    /// Initiates a dance request through the configured space-scoped initiator.
    pub async fn initiate_dance(
        self: &Arc<Self>,
        request: DanceRequest,
    ) -> Result<DanceResponse, HolonError> {
        let initiator = self.get_dance_initiator()?;
        Ok(initiator.initiate_dance(self, request).await)
    }

    /// Initiates a dance request originating from host ingress.
    ///
    /// For non-read-only ingress requests, this method enforces host mutation
    /// entry policy before dispatching the dance.
    pub async fn initiate_ingress_dance(
        self: &Arc<Self>,
        request: DanceRequest,
        is_read_only: bool,
    ) -> Result<DanceResponse, HolonError> {
        if !is_read_only {
            self.assert_allowed(TransactionOperation::HostMutationEntry)?;
        }

        self.initiate_dance(request).await
    }

    // ---------------------------------------------------------------------
    // Runtime Execution Services (formerly trait methods)
    // ---------------------------------------------------------------------

    /// Returns the holon service.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync> {
        self.space_manager.get_holon_service()
    }

    /// Returns the dance initiator.
    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError> {
        self.space_manager.get_dance_initiator()
    }

    /// Returns the current space holon reference (if any).
    ///
    /// This version no longer reacquires `Arc<TransactionContext>` through
    /// `TransactionManager`. Instead, it requires `self: &Arc<Self>` so we
    /// can mint a handle directly.
    pub fn get_space_holon(self: &Arc<Self>) -> Result<Option<HolonReference>, HolonError> {
        let maybe_holon_id = self.space_manager.get_space_holon_id()?;

        let Some(holon_id) = maybe_holon_id else {
            return Ok(None);
        };

        let handle = self.context_handle();

        Ok(Some(HolonReference::Smart(SmartReference::new_from_id(handle, holon_id))))
    }

    /// Sets the space holon id.
    pub fn set_space_holon_id(&self, space_holon_id: HolonId) -> Result<(), HolonError> {
        self.space_manager.set_space_holon_id(space_holon_id)
    }

    // ---------------------------------------------------------------------
    // Manager Access
    // ---------------------------------------------------------------------

    /// Returns a strong reference to the space manager.
    fn space_manager(&self) -> Arc<HolonSpaceManager> {
        Arc::clone(&self.space_manager)
    }

    // Public accessors for staging/transient behaviors (transaction-scoped).
    fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn HolonStagingBehavior + Send + Sync>
    }

    pub(crate) fn transient_manager_access(
        &self,
        _key: crate::reference_layer::transient_reference::TransientRefAccessKey,
    ) -> Arc<dyn TransientManagerAccess + Send + Sync> {
        Arc::clone(&self.transient_manager) as Arc<dyn TransientManagerAccess + Send + Sync>
    }

    pub(crate) fn nursery_access(
        &self,
        _key: crate::reference_layer::staged_reference::StagedRefAccessKey,
    ) -> Arc<dyn NurseryAccess + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn NurseryAccess + Send + Sync>
    }

    pub(crate) fn cache_access(
        &self,
        _key: crate::reference_layer::smart_reference::SmartRefAccessKey,
    ) -> Arc<dyn HolonCacheAccess + Send + Sync> {
        self.space_manager().get_cache_access()
    }

    // Internal privileged accessors for reference resolution.
    fn transition_to_committed_if_needed(&self) -> Result<(), HolonError> {
        match self.transition_to_committed() {
            Ok(()) => Ok(()),
            Err(HolonError::TransactionAlreadyCommitted { .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn should_transition_from_commit_response(
        &self,
        commit_response_reference: &TransientReference,
    ) -> Result<bool, HolonError> {
        let status_value = commit_response_reference
            .property_value(CorePropertyTypeName::CommitRequestStatus.as_property_name())?;

        match status_value {
            Some(BaseValue::StringValue(status)) => match status.0.as_str() {
                "Complete" => Ok(true),
                "Incomplete" => Ok(false),
                other => Err(HolonError::InvalidParameter(format!(
                    "Unexpected CommitRequestStatus value on CommitResponse: {}",
                    other
                ))),
            },
            Some(other) => Err(HolonError::InvalidType(format!(
                "CommitRequestStatus on CommitResponse must be a StringValue, found {:?}",
                other
            ))),
            None => Ok(false),
        }
    }

    fn should_transition_from_load_response(
        &self,
        load_response_reference: &TransientReference,
    ) -> Result<bool, HolonError> {
        let status_value =
            load_response_reference.property_value(CorePropertyTypeName::LoadCommitStatus.as_property_name())?;

        match status_value {
            Some(BaseValue::StringValue(status)) => match status.0.as_str() {
                "Complete" => Ok(true),
                "Incomplete" | "Skipped" => Ok(false),
                other => Err(HolonError::InvalidParameter(format!(
                    "Unexpected LoadCommitStatus value on HolonLoadResponse: {}",
                    other
                ))),
            },
            Some(other) => Err(HolonError::InvalidType(format!(
                "LoadCommitStatus on HolonLoadResponse must be a StringValue, found {:?}",
                other
            ))),
            None => Ok(false),
        }
    }

    // ---------------------------------------------------------------------
    // State Import / Export
    // ---------------------------------------------------------------------

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

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::{
        HolonCollection, RelationshipMap, ServiceRoutingPolicy,
    };
    use crate::reference_layer::{HolonServiceApi, StagedReference};
    use core_types::{HolonError, LocalId, RelationshipName};
    use std::any::Any;

    #[derive(Debug)]
    struct TestHolonService;

    fn unreachable_in_transaction_context_tests<T>() -> Result<T, HolonError> {
        Err(HolonError::NotImplemented("TestHolonService".to_string()))
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
            unreachable_in_transaction_context_tests()
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            unreachable_in_transaction_context_tests()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            unreachable_in_transaction_context_tests()
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            unreachable_in_transaction_context_tests()
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_transaction_context_tests()
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_transaction_context_tests()
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_transaction_context_tests()
        }
    }

    fn build_context() -> Arc<TransactionContext> {
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));

        space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&space_manager))
            .expect("default transaction should open")
    }

    #[test]
    fn commit_execution_guard_sets_and_releases_flag() {
        let context = build_context();
        assert!(!context.is_host_commit_in_progress());

        {
            let _guard = context
                .begin_host_commit_ingress_guard()
                .expect("guard acquisition should succeed");
            assert!(context.is_host_commit_in_progress());
        }

        assert!(!context.is_host_commit_in_progress());
    }

    #[test]
    fn commit_execution_guard_rejects_reentrant_acquire() {
        let context = build_context();
        let _guard =
            context.begin_host_commit_ingress_guard().expect("first acquisition should succeed");

        let err = context
            .begin_host_commit_ingress_guard()
            .expect_err("second acquisition while held should fail");

        assert!(matches!(err, HolonError::TransactionCommitInProgress { .. }));
    }

    #[test]
    fn commit_execution_guard_releases_on_early_error_path() {
        let context = build_context();

        let result: Result<(), HolonError> = (|| {
            let _guard = context.begin_host_commit_ingress_guard()?;
            Err(HolonError::InvalidParameter("synthetic failure".into()))
        })();

        assert!(matches!(result, Err(HolonError::InvalidParameter(_))));
        assert!(
            !context.is_host_commit_in_progress(),
            "guard must be released even when scope exits through error"
        );
    }

    #[test]
    fn external_mutation_rejected_while_host_commit_ingress_active() {
        let context = build_context();
        let _guard =
            context.begin_host_commit_ingress_guard().expect("guard acquisition should succeed");

        assert!(
            context.assert_allowed(TransactionOperation::HostMutationEntry).is_err(),
            "external mutation should be rejected while commit ingress is active"
        );
    }

    #[test]
    fn external_mutation_rejected_after_transaction_committed() {
        let context = build_context();
        context.transition_to_committed().expect("open transaction should transition to committed");

        assert!(
            context.assert_allowed(TransactionOperation::HostMutationEntry).is_err(),
            "external mutation should be rejected after committed"
        );
    }

    #[test]
    fn commit_execution_is_allowed_during_host_commit_ingress_when_open() {
        let context = build_context();
        let _guard =
            context.begin_host_commit_ingress_guard().expect("guard acquisition should succeed");

        assert!(
            context.assert_allowed(TransactionOperation::CommitExecution).is_ok(),
            "commit lifecycle check should succeed while open, even under ingress guard"
        );
    }

    #[test]
    fn commit_execution_is_rejected_after_transaction_committed() {
        let context = build_context();
        context.transition_to_committed().expect("open transaction should transition to committed");

        assert!(
            context.assert_allowed(TransactionOperation::CommitExecution).is_err(),
            "commit lifecycle check should reject committed transaction"
        );
    }

    #[test]
    fn create_transient_is_allowed_during_host_commit_ingress_when_open() {
        let context = build_context();
        let _guard =
            context.begin_host_commit_ingress_guard().expect("guard acquisition should succeed");

        assert!(
            context.assert_allowed(TransactionOperation::CreateTransient).is_ok(),
            "transient creation should remain allowed during host commit ingress"
        );
    }

    #[test]
    fn create_transient_is_allowed_after_transaction_committed() {
        let context = build_context();
        context.transition_to_committed().expect("open transaction should transition to committed");

        assert!(
            context.assert_allowed(TransactionOperation::CreateTransient).is_ok(),
            "transient creation should remain allowed after committed lifecycle state"
        );
    }
}
