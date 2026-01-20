//! Transaction-scoped execution context shell for staging and transient pools.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use core_types::HolonError;

use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transient_manager_access_internal::TransientManagerAccessInternal;
use crate::core_shared_objects::{
    HolonCacheAccess, Nursery, NurseryAccess, TransientCollection, TransientHolonManager,
    TransientManagerAccess,
};
use crate::dances::dance_initiator::DanceInitiator;
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    HolonsContextBehavior, TransientHolonBehavior,
};

use super::TxId;

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
    pub(super) fn new(tx_id: TxId, space_manager: Arc<HolonSpaceManager>) -> Self {
        // Store transaction identity and space linkage.
        // Construct the transaction-owned staging and transient pools.
        Self {
            tx_id,
            is_open: AtomicBool::new(true),
            space_manager,
            nursery: Arc::new(Nursery::new()),
            transient_manager: Arc::new(TransientHolonManager::new_empty()),
        }
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

    fn require_space_manager(&self) -> Arc<HolonSpaceManager> {
        // Space manager lifetime is guaranteed by the strong Arc stored on the context.
        Arc::clone(&self.space_manager)
    }
}

impl HolonsContextBehavior for TransactionContext {
    fn tx_id(&self) -> TxId {
        self.tx_id
    }

    fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Acquire)
    }

    fn get_nursery_access(&self) -> Arc<dyn NurseryAccess + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn NurseryAccess + Send + Sync>
    }

    fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync> {
        Arc::clone(&self.nursery) as Arc<dyn HolonStagingBehavior + Send + Sync>
    }

    fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.nursery.export_staged_holons()
    }

    fn import_staged_holons(&self, staged_holons: SerializableHolonPool) {
        // Preserve the existing void API by discarding the internal result.
        let _ = self.nursery.import_staged_holons(staged_holons);
    }

    fn get_transient_behavior_service(&self) -> Arc<dyn TransientHolonBehavior + Send + Sync> {
        Arc::clone(&self.transient_manager) as Arc<dyn TransientHolonBehavior + Send + Sync>
    }

    fn get_transient_manager_access(&self) -> Arc<dyn TransientManagerAccess + Send + Sync> {
        Arc::clone(&self.transient_manager) as Arc<dyn TransientManagerAccess + Send + Sync>
    }

    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError> {
        self.transient_manager.export_transient_holons()
    }

    fn import_transient_holons(&self, transient_holons: SerializableHolonPool) {
        // Preserve the existing void API by discarding the internal result.
        let _ = self.transient_manager.import_transient_holons(transient_holons);
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

    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError> {
        // Transaction-scoped mapping from stored id to runtime reference.
        let space_holon_id = self.require_space_manager().get_space_holon_id()?;
        Ok(space_holon_id.map(HolonReference::from))
    }

    fn set_space_holon(&self, space: HolonReference) -> Result<(), HolonError> {
        // Validation: only persisted (smart) holons may anchor the space.
        let reference_kind = space.reference_kind_string();
        let reference_id = space.reference_id_string();
        let space_holon_id = match space {
            HolonReference::Smart(smart_reference) => smart_reference.get_id()?,
            _ => {
                return Err(HolonError::InvalidHolonReference(format!(
                    "Space holon must be a SmartReference; got {} ({})",
                    reference_kind, reference_id
                )));
            }
        };

        self.require_space_manager().set_space_holon_id(space_holon_id)
    }

    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>> {
        self.require_space_manager().get_transient_state()
    }
}
