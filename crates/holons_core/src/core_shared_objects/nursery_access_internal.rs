use crate::core_shared_objects::holon_pool::SerializableHolonPool;
use crate::core_shared_objects::{Holon, HolonError, NurseryAccess};
use crate::HolonStagingBehavior;
use shared_types_holon::holon_node::TemporaryId;
use shared_types_holon::MapString;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// Provides **internal management** of staged holons in the nursery.
///
/// This trait is used **only by the nursery itself and HolonSpaceManager**.
/// It defines methods for:
/// - **Clearing staged holons**
/// - **Retrieving holons by key**
/// - **Directly staging new holons**
pub trait NurseryAccessInternal: NurseryAccess + HolonStagingBehavior {
    /// Enables safe downcasting of `NurseryAccessInternal` trait objects to their concrete type.
    ///
    /// This method is useful when working with `NurseryAccessInternal` as a trait object (`dyn NurseryAccessInternal`)
    /// but needing to recover its underlying concrete type (e.g., `Nursery`). It allows casting
    /// through `Any`, which is required because Rust does not support direct downcasting of trait objects.
    fn as_any(&self) -> &dyn Any;

    /// # CAUTION!!!
    ///
    /// **This method is ONLY intended for use by the GuestHolonService**
    ///
    /// Clears the Nursery's staged holons
    fn clear_stage(&mut self);

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

    /// Exports the currently staged holons as a `SerializableHolonPool`.
    ///
    /// This method creates a **deep clone** of the current `HolonPool`, including all holons
    /// and the keyed index. The returned `SerializableHolonPool` is **independent** of the original,
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
    /// A `SerializableHolonPool` containing a **deep clone** of the current staged holons and their keyed index.
    fn export_staged_holons(&self) -> SerializableHolonPool;

    /// Imports a `SerializableHolonPool`, replacing the current staged holons.
    ///
    /// This method **completely replaces** the current staged holons with the provided `SerializableHolonPool`.
    /// Any existing staged holons will be **discarded** in favor of the imported data.
    ///
    /// # Use Cases
    /// - **Client-Guest Syncing:** Allows the client to **restore** a nursery state previously exported.
    /// - **State Restoration:** Enables reloading staged holons from a saved state.
    ///
    /// # Notes
    /// - The method ensures that **holons are correctly wrapped in `Rc<RefCell<_>>`** upon import.
    /// - If the provided pool is empty, the `Nursery` will also be cleared.
    ///
    /// # Arguments
    /// - `pool` - A `SerializableHolonPool` containing the staged holons and their keyed index.
    fn import_staged_holons(&mut self, pool: SerializableHolonPool) -> ();

    /// Provides direct access to the staged Holons in the Nursery's HolonPool.
    ///
    /// This method returns a reference to the underlying collection of staged Holons,
    /// allowing commit functions to operate on the actual Holon instances without cloning.
    ///
    /// # Returns
    ///
    /// A Ref to a `Vec<Rc<RefCell<Holon>>>` containing all staged Holons.
    // fn get_holons_to_commit(&self) -> impl Iterator<Item = Rc<RefCell<Holon>>> + '_;
    fn get_holons_to_commit(&self) -> Vec<Rc<RefCell<Holon>>>;

    // /// Stages a new holon and optionally updates the keyed index.
    // ///
    // /// # Arguments
    // /// * `holon` - A reference to the holon to be staged.
    // ///
    // /// # Returns
    // /// The index of the staged holon in the nursery.
    // fn stage_holon(&self, holon: Holon) -> usize;
}
