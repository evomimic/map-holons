
use crate::core_shared_objects::holon::TransientHolon;
use crate::reference_layer::{TransientReference};
use base_types::MapString;
use core_types::HolonError;
use integrity_core_types::HolonNodeModel;

/// Defines **high-level transient behavior**, abstracting away direct transient_manager operations.
///
/// This trait is intended for use by **test cases, API consumers, and higher-level logic**.
/// It provides a structured way to:
/// - **Create new transient holons**
/// - **Retrieve transient holons by key**
/// - **Commit or abandon transient changes**
///
/// This trait does **not** expose low-level details.
///
/// Base key represents the Holon's key independent of versioning.
pub trait TransientHolonBehavior {

    // ===========================
    // TransientHolon Constructors
    // ===========================

    fn create_empty(&self) -> Result<TransientReference, HolonError>;

    fn create_from_model(
        &self,
        model: HolonNodeModel,
    ) -> Result<TransientReference, HolonError>;

    fn create_immutable(&self) -> Result<TransientReference, HolonError>;

    // =======
    //  WRITE
    // =======

    /// Adds the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the transient holon
    /// to be retrieved by key.
    fn add_new_holon(&self, holon: TransientHolon) -> Result<TransientReference, HolonError>;

    // ======
    //  READ
    // ======

    /// Convenience method for retrieving a single TransientReference for a base key, when the caller expects there to only be one.
    /// Returns a duplicate error if multiple found.
    fn get_transient_holon_by_base_key(&self, key: &MapString) -> Result<TransientReference, HolonError>;

    /// Returns TransientReference's for all Holons that have the same base key.
    /// This can be useful if multiple versions of the same Holon are being transient at the same time.
    fn get_transient_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<TransientReference>, HolonError>;

    /// Does a lookup by full (unique) key on transient holons.
    fn get_transient_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError>;

    /// Returns a count of the number of transient holons.
    fn transient_count(&self) -> i64;
}
