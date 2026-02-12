use std::any::Any;

use super::holon_pool::HolonPool;
use crate::{HolonStagingBehavior, NurseryAccess, StagedReference};
use base_types::MapString;
use core_types::{HolonError, TemporaryId};

/// Provides thread-safe **internal access** to staged holons within the `Nursery`.
///
/// This trait extends [`NurseryAccess`] and [`HolonStagingBehavior`] and is implemented
/// by components that manage the lifecycle of staged holons—primarily the `Nursery`
/// and `HolonSpaceManager`.
///
/// > **Note:** Although this trait is thread-safe (`Send + Sync`), it is **intended only for internal use**
/// by core components and should not be exposed as part of public or client-facing APIs.
///
/// ### Responsibilities:
/// - Clearing and replacing staged holons
/// - Accessing holons by base or versioned key
/// - Exporting/importing the full staged holon pool
/// - Providing holons to the commit pipeline
pub trait NurseryAccessInternal: NurseryAccess + HolonStagingBehavior + Send + Sync {
    /// Enables safe downcasting of `NurseryAccessInternal` trait objects to their
    /// concrete type when core services require direct access to `Nursery`.
    ///
    /// This method is useful when working with `NurseryAccessInternal` as a trait object
    /// (`dyn NurseryAccessInternal`) but needing to recover its underlying concrete type
    /// (e.g., `Nursery`). It allows casting through `Any`, which is required because Rust does not
    /// support direct downcasting of trait objects.
    fn as_any(&self) -> &dyn Any;

    /// # Safety -- This method is intended **only for internal use by `GuestHolonService`**.
    ///
    /// It clears all staged holons and should not be exposed outside the core commit lifecycle.
    fn clear_stage(&self) -> Result<(), HolonError>;

    /// Finds a holon by its (unique) versioned key and returns its TemporaryId.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// `Ok(TemporaryId)` containing the index if the key exists, or an `Err` if the key is not found.
    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError>;

    /// Exports the currently staged holons as a runtime `HolonPool`.
    ///
    /// This method creates a **deep clone** of the current `HolonPool`, including all holons
    /// and the keyed index. The returned `HolonPool` is **independent** of the original,
    /// meaning any modifications to it will **not affect** the actual `Nursery` state.
    ///
    /// # Use Cases
    /// - **Client-Guest Syncing:** Intended for **ping-ponging nursery state** between the client and guest.
    /// - **Serialization:** Facilitates serialization for storage, transmission, or debugging.
    ///
    /// # Notes
    /// - The cloning process is **optimized** but may have a cost if holons contain large data.
    /// - **Internal references within the exported data remain consistent**, ensuring accurate reconstruction upon import.
    ///
    /// # Returns
    /// A `HolonPool` containing a **deep clone** of the current staged holons and their keyed index.
    fn export_staged_holons(&self) -> Result<HolonPool, HolonError>;

    /// Imports a bound `HolonPool`, replacing the current staged holons.
    ///
    /// This method **completely replaces** the current staged holons with the provided `HolonPool`.
    /// Any existing staged holons will be **discarded** in favor of the imported data.
    ///
    /// # Use Cases
    /// - **Client-Guest Syncing:** Allows the client to **restore** a nursery state previously exported.
    /// - **State Restoration:** Enables reloading staged holons from a saved state.
    ///
    /// # Notes
    /// - The method ensures that **holons are correctly wrapped in `Arc<RwLock<Holon>>`** upon import.
    /// - If the provided pool is empty, the `Nursery` will also be cleared.
    ///
    /// # Arguments
    /// - `pool` - A `HolonPool` containing the staged holons and their keyed index.
    fn import_staged_holons(&self, pool: HolonPool) -> Result<(), HolonError>;

    /// Returns a reference-layer view of all staged holons as `StagedReference`s.
    /// - This method assumes the provided `HolonPool` is already correctly
    ///   constructed; wrapping holons in `Arc<RwLock<Holon>>` is handled by
    ///   `HolonPool` itself, not by this trait.
    fn get_staged_references(
        &self,
        //transaction_handle: &TransactionContextHandle,
    ) -> Result<Vec<StagedReference>, HolonError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_thread_safe<T: Send + Sync>() {}

    #[test]
    fn nursery_access_internal_is_thread_safe() {
        trait Dummy: NurseryAccessInternal {}
        impl<T: NurseryAccessInternal> Dummy for T {}
        assert_thread_safe::<&dyn Dummy>();
    }
}
