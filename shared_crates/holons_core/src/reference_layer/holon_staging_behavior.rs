use crate::{
    reference_layer::{StagedReference, TransientReference},
    HolonReference, HolonsContextBehavior, SmartReference,
};

use crate::core_shared_objects::transactions::TransactionContextHandle;
use base_types::MapString;
use core_types::HolonError;

/// Defines **high-level staging behavior**, abstracting away direct nursery operations.
///
/// This trait is intended for use by **test cases, API consumers, and higher-level logic**.
/// It provides a structured way to:
/// - **Stage new holons**
/// - **Retrieve staged holons by key**
/// - **Commit or abandon staged changes**
///
/// This trait does **not** expose low-level details.
///
/// Base key represents the Holon's key independent of versioning.
pub trait HolonStagingBehavior: Send + Sync {
    /// Convenience method for retrieving a single StagedReference for a base key, when the caller expects there to only be one.
    /// Returns a duplicate error if multiple found.
    fn get_staged_holon_by_base_key(
        &self,
        //transaction_handle: &TransactionContextHandle,
        key: &MapString,
    ) -> Result<StagedReference, HolonError>;

    /// Returns StagedReference's for all Holons that have the same base key.
    /// This can be useful if multiple versions of the same Holon are being staged at the same time.
    fn get_staged_holons_by_base_key(
        &self,
        //transaction_handle: &TransactionContextHandle,
        key: &MapString,
    ) -> Result<Vec<StagedReference>, HolonError>;

    /// Does a lookup by full (unique) key on staged holons.
    fn get_staged_holon_by_versioned_key(
        &self,
        //transaction_handle: &TransactionContextHandle,
        key: &MapString,
    ) -> Result<StagedReference, HolonError>;

    /// Returns a count of the number of holons being staged
    fn staged_count(&self) -> Result<i64, HolonError>;

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_holon(
        &self,
        //transaction_handle: &TransactionContextHandle,
        holon: TransientReference,
    ) -> Result<StagedReference, HolonError>;

    /// Stage a new holon as a clone of an existing holon, with a new key.
    fn stage_new_from_clone(
        &self,
        //transaction_handle: &TransactionContextHandle,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError>;

    /// Stage a new version of the current holon, maintaining lineage.
    fn stage_new_version(
        &self,
        //transaction_handle: &TransactionContextHandle,
        current_version: SmartReference,
    ) -> Result<StagedReference, HolonError>;
}
