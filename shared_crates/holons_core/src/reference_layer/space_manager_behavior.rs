use core_types::{HolonError, HolonId};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::transient_collection::TransientCollection;
use crate::core_shared_objects::transactions::TransactionManager;
use crate::dances::dance_initiator::DanceInitiator;
use crate::reference_layer::HolonServiceApi;
use std::sync::{Arc, RwLock};

/// Defines the core behavior of a **Holon Space**, providing:
/// 1. **Space-scoped services** (Cache, HolonService, DanceInitiator)
/// 2. **Space identity** (local space holon reference)
/// 3. **Transient collections** (temporary, non-transactional state)
/// 4. The **TransactionManager** authority for creating transaction contexts.
///
/// Transaction-scoped behavior is exposed via `HolonsContextBehavior`.
pub trait HolonSpaceBehavior {
    /// Provides access to the **cache service** for retrieving and storing holons.
    ///
    /// The cache mediates access between:
    /// - The **local cache** (fast retrieval of recently accessed holons)
    /// - Outbound proxies (retrieving holons from other spaces, if supported)
    ///
    /// # Returns
    /// - An `Arc<dyn HolonCacheAccess + Send + Sync>` that allows cache operations.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess + Send + Sync>;

    /// Retrieves the configured [`DanceInitiator`] responsible for outbound Dances.
    ///
    /// # Returns
    /// - `Ok(Arc<dyn DanceInitiator>)` when the initiator has been injected.
    /// - `Err(HolonError::ServiceNotAvailable("DanceInitiator"))` when the
    ///   initiator is not configured for this context (e.g., in a guest runtime).
    fn get_dance_initiator(&self) -> Result<Arc<dyn DanceInitiator>, HolonError>;

    /// Provides access to the **holon service API**, which includes core operations
    /// such as creating, retrieving, updating, and deleting holons.
    ///
    /// # Returns
    /// - An `Arc<dyn HolonServiceApi + Send + Sync>` for interacting with holons.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi + Send + Sync>;

    /// Retrieves the **local space holon id**, if it exists.
    ///
    /// The **space holon** represents the current holon space and serves as an anchor
    /// for all holon operations within this space.
    ///
    /// The space holon:
    /// - Is `None` only during a brief initialization window.
    /// - Is `Some(HolonId)` in steady state.
    /// - Returns `Err(HolonError::FailedToAcquireLock(_))` if the internal lock
    ///   cannot be acquired, indicating possible corruption or poisoning.
    fn get_space_holon_id(&self) -> Result<Option<HolonId>, HolonError>;

    /// Provides access to a **transient state collection**, initializing it if necessary.
    ///
    /// The transient state:
    /// - Stores temporary collections of holons that do **not require persistence**.
    /// - Can be used to hold query results, temporary groupings, or working sets.
    ///
    /// # Behavior
    /// - If the transient state has **not been initialized**, it is created automatically.
    ///
    /// # Returns
    /// - An `Arc<RwLock<TransientCollection>>` for managing transient holon
    ///   collections in a thread-safe context.
    fn get_transient_state(&self) -> Arc<RwLock<TransientCollection>>;

    /// Updates the local space holon id.
    ///
    /// # Arguments
    /// - `space` - The new `HolonId` for the space.
    /// # Errors
    /// Returns `HolonError::FailedToAcquireLock` if the internal write lock
    /// on `local_holon_space` cannot be acquired.
    fn set_space_holon_id(&self, space: HolonId) -> Result<(), HolonError>;

    /// Provides access to the per-space transaction manager.
    ///
    /// The transaction manager is the authority for creating and registering
    /// transaction contexts for this space.
    fn get_transaction_manager(&self) -> Arc<TransactionManager>;
}
