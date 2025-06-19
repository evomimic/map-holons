use crate::core_shared_objects::TransientManagerAccess;
use crate::reference_layer::{HolonReference, HolonServiceApi, HolonStagingBehavior};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::nursery_access::NurseryAccess;
use crate::HolonCollectionApi;

use std::cell::RefCell;
use std::sync::Arc;

/// Defines the core behavior of a **Holon Space**, providing:
/// 1. **Service registry access** (Nursery, Cache, HolonService, Transient State)
/// 2. **Controlled mediation** for importing/exporting staged holons.
pub trait HolonSpaceBehavior {
    /// Provides access to the **cache service** for retrieving and storing holons.
    ///
    /// The cache mediates access between:
    /// - The **local cache** (fast retrieval of recently accessed holons)
    /// - Outbound proxies (retrieving holons from other spaces, if supported)
    ///
    /// # Returns
    /// - An `Arc<dyn HolonCacheAccess>` that allows cache operations.
    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess>;

    /// Provides access to the **holon service API**, which includes core operations
    /// such as creating, retrieving, updating, and deleting holons.
    ///
    /// # Returns
    /// - An `Arc<dyn HolonServiceApi>` for interacting with holons.
    fn get_holon_service(&self) -> Arc<dyn HolonServiceApi>;

    /// Provides access to the **nursery**, where staged holons are temporarily stored before being committed.
    ///
    /// The **nursery** allows:
    /// - Staging new holons
    /// - Accessing holons that are not yet persisted
    /// - Managing relationships within staged holons
    ///
    /// # Behavior
    /// - If the nursery is **not yet initialized**, it will be created automatically.
    ///
    /// # Returns
    /// - An `Arc<RefCell<dyn NurseryAccess>>` to allow interior mutability.
    fn get_nursery_access(&self) -> Arc<RefCell<dyn NurseryAccess>>;

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
    /// This allows holons to be staged, retrieved, and committed within the nursery.
    ///
    /// # Returns
    /// - An `Arc<RefCell<dyn HolonStagingBehavior>>` for interacting with staged holons.
    fn get_staging_behavior_access(&self) -> Arc<RefCell<dyn HolonStagingBehavior>>;

    /// Provides access to the **TransientHolonManager**, where transient holons are stored.
    ///
    /// The **TransientHolonManager** allows:
    /// - Accessing holons that are not yet staged
    /// - Managing relationships within transient holons
    ///
    /// # Behavior
    /// - If the manager is **not yet initialized**, it will be created automatically.
    ///
    /// # Returns
    /// - An `Arc<RefCell<dyn TransientManagerAccess>>` to allow interior mutability.
    fn get_transient_manager_access(&self) -> Arc<RefCell<dyn TransientManagerAccess>>;

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
    /// - An `Arc<RefCell<dyn HolonCollectionApi>>` for managing transient holon collections.
    fn get_transient_state(&self) -> Arc<RefCell<dyn HolonCollectionApi>>;

    /// **Mediates access to nursery exports, avoiding direct exposure in `NurseryAccess`.**
    ///
    /// This method is used when **sending the staged state** to another process (e.g., guest → client sync).
    ///
    /// # Returns
    /// - A `SerializableHolonPool` containing all staged holons and their keyed index.
    fn export_staged_holons(&self) -> SerializableHolonPool;

    /// **Mediates access to transient exports, avoiding direct exposure in `TransientManagerAccess`.**
    ///
    /// This method is used when ** sending the "transient state" ** to another process (e.g., guest → client sync).
    ///
    /// # Returns
    /// - A `SerializableHolonPool` containing all transient holons and their keyed index.
    fn export_transient_holons(&self) -> SerializableHolonPool;

    /// **Mediates import of staged holons to prevent direct modification via `NurseryAccess`.**
    ///
    /// This method **replaces** the existing staged holons in the nursery with the provided data.
    ///
    /// # Arguments
    /// - `staged_holons` - A `SerializableHolonPool` containing holons and their keyed index.
    fn import_staged_holons(&self, staged_holons: SerializableHolonPool);

    /// **Mediates import of transient holons to prevent direct modification via `TransientManagerAccess`.**
    ///
    /// This method **replaces** the existing transient holons in the transient_manager with the provided data.
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
