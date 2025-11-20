use core_types::HolonError;

use crate::reference_layer::{HolonReference, HolonServiceApi};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::holon_pool::SerializableHolonPool;

// Import thread-safe core objects
use crate::core_shared_objects::{TransientCollection, TransientManagerAccess};
use crate::{HolonStagingBehavior, NurseryAccess, TransientHolonBehavior};

use crate::dances::dance_initiator::DanceInitiator;
use std::sync::{Arc, RwLock};

/// Defines the core behavior of a **Holon Space**, providing:
/// 1. **Service registry access** (Staging/Nursery, Cache, HolonService, Transient Manager/State)
/// 2. **Controlled mediation** for importing/exporting staged and transient holons.
/// 3. A stable entry point for higher-level reference-layer and loader operations.
pub trait HolonSpaceBehavior {
    /// **Mediates access to nursery exports, avoiding direct exposure in `NurseryAccess`.**
    ///
    /// This method is used when **sending the staged state** to another process
    /// (e.g., guest → client sync).
    ///
    /// # Returns
    /// - A `SerializableHolonPool` containing all staged holons and their keyed index.
    fn export_staged_holons(&self) -> Result<SerializableHolonPool, HolonError>;

    /// **Mediates access to transient exports, avoiding direct exposure in `TransientManagerAccess`.**
    ///
    /// This method is used when **sending the transient state** to another process
    /// (e.g., guest → client sync).
    ///
    /// # Returns
    /// - A `SerializableHolonPool` containing all transient holons and their keyed index.
    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError>;

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

    /// Provides access to the **nursery access API**, where staged holons are
    /// temporarily stored before being committed.
    ///
    /// The **nursery** allows:
    /// - Staging new holons
    /// - Accessing holons that are not yet persisted
    /// - Managing relationships within staged holons
    ///
    /// # Behavior
    /// - If the nursery is **not yet initialized**, it will be created automatically
    ///   by the `HolonSpaceManager` implementation.
    ///
    /// # Returns
    /// - An `Arc<dyn NurseryAccess + Send + Sync>`; the underlying implementation
    ///   is internally thread-safe and handles its own locking.
    fn get_nursery_access(&self) -> Arc<dyn NurseryAccess + Send + Sync>;

    /// Retrieves a reference to the **local space holon**, if it exists.
    ///
    /// The **space holon** represents the current holon space and serves as an anchor
    /// for all holon operations within this space.
    ///
    /// # Returns
    /// - `Some(HolonReference)` if the local space holon is set.
    /// - `None` if the space holon is not available.
    fn get_space_holon(&self) -> Option<HolonReference>;

    /// Provides access to a **component that implements the `HolonStagingBehavior` API**.
    ///
    /// This behavior API is responsible for:
    /// - Staging holons
    /// - Looking up staged holons by key/id
    /// - Committing staged holons into saved state
    ///
    /// The underlying staging manager (`Nursery` / staged-holon manager) is
    /// internally synchronized; callers do not need to manage any outer locks.
    ///
    /// # Returns
    /// - An `Arc<dyn HolonStagingBehavior + Send + Sync>` for interacting
    ///   with staged holons in a thread-safe manner.
    fn get_staging_service(&self) -> Arc<dyn HolonStagingBehavior + Send + Sync>;

    /// Provides the service for the **component that implements the `TransientHolonBehavior` API**.
    ///
    /// This behavior API is responsible for:
    /// - Creating new transient holons
    /// - Looking up and updating transient holons
    ///
    /// The underlying `TransientHolonManager` is internally synchronized; callers
    /// interact through this behavior trait and do not manage locks directly.
    ///
    /// # Returns
    /// - An `Arc<dyn TransientHolonBehavior + Send + Sync>` for interacting
    ///   with transient holons in a thread-safe manner.
    fn get_transient_behavior_service(&self) -> Arc<dyn TransientHolonBehavior + Send + Sync>;

    /// Provides access to the **TransientHolonManager** via its access API.
    ///
    /// The **TransientHolonManager** allows:
    /// - Accessing holons that are not yet staged
    /// - Managing relationships within transient holons
    ///
    /// # Behavior
    /// - If the manager is **not yet initialized**, it will be created automatically
    ///   by the `HolonSpaceManager` implementation.
    ///
    /// # Returns
    /// - An `Arc<dyn TransientManagerAccess + Send + Sync>`; the underlying
    ///   manager is internally synchronized and handles its own locking.
    fn get_transient_manager_access(&self) -> Arc<dyn TransientManagerAccess + Send + Sync>;

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

    /// **Mediates import of staged holons to prevent direct modification via `NurseryAccess`.**
    ///
    /// This method **replaces** the existing staged holons in the nursery with the provided data.
    /// The underlying nursery implementation is responsible for synchronizing updates
    /// to its internal pool.
    ///
    /// # Arguments
    /// - `staged_holons` - A `SerializableHolonPool` containing holons and their keyed index.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool);

    /// **Mediates import of transient holons to prevent direct modification via `TransientManagerAccess`.**
    ///
    /// This method **replaces** the existing transient holons in the transient manager with
    /// the provided data. The underlying manager implementation is responsible for
    /// synchronizing updates to its internal pool.
    ///
    /// # Arguments
    /// - `transient_holons` - A `SerializableHolonPool` containing holons and their keyed index.
    fn import_transient_holons(&self, transient_holons: SerializableHolonPool);

    /// Updates the local space holon reference.
    ///
    /// # Arguments
    /// - `space` - The new `HolonReference` for the space.
    fn set_space_holon(&self, space: HolonReference);
}
