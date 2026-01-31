use core_types::{HolonError, HolonId};
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::core_shared_objects::transactions::TxId;
use crate::core_shared_objects::{HolonCacheAccess, TransientCollection};
use crate::dances::dance_initiator::DanceInitiator;
use crate::reference_layer::{HolonReference, HolonServiceApi};

/// Defines the execution surface for a single transaction within a space.
pub trait HolonsContextBehavior: Debug + Send + Sync {
    /// Returns the transaction id for this context.
    fn tx_id(&self) -> TxId;

    /// Returns whether the transaction is still open.
    fn is_open(&self) -> bool;

    /// Provides access to the cache service for the space.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync>;

    /// Provides access to the holon service API for the space.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync>;

    /// Retrieves the dance initiator for the space.
    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError>;

    /// Retrieves the local space holon reference.
    fn get_space_holon(&self) -> Result<Option<HolonReference>, HolonError>;

    /// Updates the local space holon reference id.
    fn set_space_holon_id(&self, space: HolonId) -> Result<(), HolonError>;

    /// Provides access to the transient state collection.
    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>>;
}
