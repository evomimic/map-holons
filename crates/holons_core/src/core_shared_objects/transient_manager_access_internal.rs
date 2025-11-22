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

/// Internal interface for **direct management** of transient holons.
///
/// This trait is implemented **only by `TransientHolonManager`** and is used internally
/// by the **HolonSpaceManager** (and occasionally subsystem code) that needs privileged,
/// low-level access to the transient holon pool.
///
/// ## Responsibilities
/// - Clearing all transient holons  
/// - Importing/exporting transient holon state  
/// - Retrieving holons by versioned key  
/// - Providing internal access to the underlying holon pool
///
/// ## Concurrency Model
/// The `TransientHolonManager` owns an **internal** `RwLock<TransientHolonPool>`.  
/// All lock acquisition is handled **inside the manager**, and all errors are surfaced as
/// `HolonError::FailedToAcquireLock`.  
///
/// **Callers must never acquire their own locks**—this trait abstracts that away entirely.
///
/// ## Notes
/// - These methods are *not* part of the public reference-layer API.  
/// - They provide privileged control over transient lifecycle and are not intended for
///   general application-level use.
pub trait TransientManagerAccessInternal:
    TransientManagerAccess + TransientHolonBehavior + Send + Sync
{
    /// Enables safe downcasting of `TransientManagerAccessInternal` trait objects to their
    /// concrete type (`TransientHolonManager`).
    ///
    /// This is needed when working with boxed trait objects and is consistent with how other
    /// internal management traits (e.g., staging) expose their concrete instance.
    fn as_any(&self) -> &dyn Any;

    /// Clears the TransientHolonManager’s pool of transient holons.
    ///
    /// # Errors
    /// Returns `HolonError::FailedToAcquireLock` if the internal write lock cannot be acquired.
    ///
    /// # Important
    /// This method fully wipes all transient holons and is used by the guest runtime during
    /// synchronization or reset operations. It is *not* part of the public holon operations API.
    fn clear_pool(&self) -> Result<(), HolonError>;

    /// Finds a holon by its unique **versioned key** and returns its `TemporaryId`.
    ///
    /// # Errors
    /// - `HolonError::FailedToAcquireLock` if the pool's internal lock cannot be acquired  
    /// - Any key lookup errors from the underlying pool
    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError>;

    /// Exports the transient holon pool as a `SerializableHolonPool`.
    ///
    /// This is a **deep clone** of the current transient state, suitable for:
    /// - Client ↔ Guest synchronization  
    /// - Serialization  
    /// - Snapshots for debugging  
    ///
    /// # Errors
    /// Returns `HolonError::FailedToAcquireLock` if the internal read lock cannot be acquired.
    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError>;

    /// Imports a transient holon pool, **replacing the current one entirely**.
    ///
    /// All existing transient holons are discarded in favor of the provided pool.
    ///
    /// # Notes
    /// The imported data is wrapped with fresh `Arc<RwLock<Holon>>` handles to preserve
    /// thread-safe interior mutability.
    ///
    /// # Errors
    /// Returns `HolonError::FailedToAcquireLock` if the internal write lock cannot be acquired.
    fn import_transient_holons(&self, pool: SerializableHolonPool) -> Result<(), HolonError>;

    /// Provides direct access to the underlying transient holon instances.
    ///
    /// This returns the actual `Arc<RwLock<Holon>>` objects stored inside the manager.
    /// It is used internally by commit routines and sync mechanisms that need to mutate or
    /// inspect holons *in place*.
    ///
    /// # Warning
    /// This is **not** part of the public reference-layer API.  
    /// External code should interact through `TransientHolonBehavior` instead of touching
    /// raw handles.
    ///
    /// # Errors
    /// Returns `HolonError::FailedToAcquireLock` if the internal read lock cannot be acquired.
    fn get_transient_holons_pool(&self) -> Result<Vec<Arc<RwLock<Holon>>>, HolonError>;
}
