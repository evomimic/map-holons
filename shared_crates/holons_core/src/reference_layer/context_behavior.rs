use async_trait::async_trait;
use core_types::HolonError;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::transactions::TxId;
use crate::core_shared_objects::transient_manager_access::TransientManagerAccess;
use crate::core_shared_objects::{HolonCacheAccess, TransientCollection};
use crate::dances::dance_initiator::DanceInitiator;
use crate::reference_layer::{
    HolonReference, HolonServiceApi, HolonSpaceBehavior, HolonStagingBehavior,
    TransientHolonBehavior,
};
use crate::NurseryAccess;

#[async_trait]
/// Defines the execution surface for a single transaction within a space.
pub trait HolonsContextBehavior: Debug + Send + Sync {
    /// Returns the transaction id for this context.
    fn tx_id(&self) -> TxId;

    /// Returns whether the transaction is still open.
    fn is_open(&self) -> bool;

    /// Provides access to the transaction-owned nursery.
    ///
    /// The nursery stores staged holons prior to commit.
    fn get_nursery_access(&self) -> Arc<dyn NurseryAccess + Send + Sync>;

    /// Provides access to the staging behavior service.
    fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync>;

    /// Exports staged holons from the transaction.
    fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError>;

    /// Imports staged holons into the transaction.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool);

    /// Provides access to the transient holon behavior service.
    fn get_transient_behavior_service(&self) -> Arc<dyn TransientHolonBehavior + Send + Sync>;

    /// Provides access to the transient holon manager.
    fn get_transient_manager_access(&self) -> Arc<dyn TransientManagerAccess + Send + Sync>;

    /// Exports transient holons from the transaction.
    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError>;

    /// Imports transient holons into the transaction.
    fn import_transient_holons(&self, transient_holons: SerializableHolonPool);

    /// Provides access to the cache service for the space.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync>;

    /// Provides access to the holon service API for the space.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync>;

    /// Retrieves the dance initiator for the space.
    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError>;

    /// Retrieves the local space holon reference.
    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError>;

    /// Updates the local space holon reference.
    fn set_space_holon(&self, space: HolonReference) -> Result<(), HolonError>;

    /// Provides access to the transient state collection.
    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>>;
}
