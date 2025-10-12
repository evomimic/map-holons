use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use base_types::MapString;
use core_types::{HolonError, TemporaryId};

use crate::{
    core_shared_objects::{
        holon::Holon, holon_pool::SerializableHolonPool, TransientManagerAccess,
    },
    reference_layer::TransientHolonBehavior,
};

/// Provides **internal management** of transient holons in the TransientHolonManager.
///
/// This trait is used **only by the TransientHolonManager itself and HolonSpaceManager**.
/// It defines methods for:
/// - **Clearing transient holons**
/// - **Retrieving holons by key**
pub trait TransientManagerAccessInternal:
    TransientManagerAccess + TransientHolonBehavior + Send + Sync
{
    /// Enables safe downcasting of `TransientManagerAccessInternal` trait objects to their concrete type.
    ///
    /// This method is useful when working with `TransientManagerAccessInternal` as a trait object (`dyn TransientManagerAccessInternal`)
    /// but needing to recover its underlying concrete type (e.g., `TransientHolonManager`). It allows casting
    /// through `Any`, which is required because Rust does not support direct downcasting of trait objects.
    fn as_any(&self) -> &dyn Any;

    /// # CAUTION!!!
    ///
    /// **This method is ONLY intended for use by the GuestHolonService**
    ///
    /// Clears the TransientHolonManager's pool of transient holons.
    fn clear_pool(&mut self);

    /// Finds a holon by its (unique) versioned key and returns its TemporaryId.
    ///
    /// # Arguments
    /// * `key` - The key to search for.
    ///
    /// # Returns
    /// `Ok(TemporaryId)` containing the index if the key exists, or an `Err` if the key is not found.
    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError>;

    /// Exports the current transient 'HolonPool' as a `SerializableHolonPool`.
    ///
    /// This method creates a **deep clone** of the current `HolonPool`, including all holons
    /// and the keyed index. The returned `SerializableHolonPool` is **independent** of the original,
    /// meaning any modifications to it will **not affect** the actual `TransientHolonManager` state.
    ///
    /// # Use Cases
    /// - **Client-Guest Syncing:** Intended for **ping-ponging TransientHolonManager state** between the client and guest.
    /// - **Serialization:** Facilitates serialization for storage, transmission, or debugging.
    ///
    /// # Notes
    /// - The cloning process is **optimized** but may have a cost if holons contain large data.
    /// - **Internal references within the exported data remain consistent**, ensuring accurate reconstruction upon import.
    ///
    /// # Returns
    /// A `SerializableHolonPool` containing a **deep clone** of the current transient holons and their keyed index.
    fn export_transient_holons(&self) -> SerializableHolonPool;

    /// Imports a `SerializableHolonPool`, replacing the current transient holons.
    ///
    /// This method **completely replaces** the current transient holons with the provided `SerializableHolonPool`.
    /// Any existing transient holons will be **discarded** in favor of the imported data.
    ///
    /// # Use Cases
    /// - **Client-Guest Syncing:** Allows the client to **restore** a TransientHolonManager state previously exported.
    /// - **State Restoration:** Enables reloading transient holons from a saved state.
    ///
    /// # Notes
    /// - The method ensures that **holons are correctly wrapped in `Arc<RwLock<Holon>>`** upon import.
    /// - If the provided pool is empty, the `TransientHolonManager` will also be cleared.
    ///
    /// # Arguments
    /// - `pool` - A `SerializableHolonPool` containing the transient holons and their keyed index.
    fn import_transient_holons(&mut self, pool: SerializableHolonPool) -> ();

    /// Provides direct access to the transient Holons in the TransientHolonManager's HolonPool.
    ///
    /// This method returns a reference to the underlying collection of transient Holons,
    /// allowing functions to operate on the actual Holon instances without cloning.
    ///
    /// # Returns
    /// A `Vec<Arc<RwLock<Holon>>>` containing all transient Holons.
    fn get_transient_holons_pool(&self) -> Vec<Arc<RwLock<Holon>>>;
}
