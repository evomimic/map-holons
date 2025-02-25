use crate::reference_layer::StagedReference;

use crate::core_shared_objects::{Holon, HolonError};
use shared_types_holon::MapString;


/// Defines **high-level staging behavior**, abstracting away direct nursery operations.
///
/// This trait is intended for use by **test cases, API consumers, and higher-level logic**.
/// It provides a structured way to:
/// - **Stage new holons**
/// - **Retrieve staged holons by key**
/// - **Commit or abandon staged changes**
///
/// This trait does **not** expose low-level details.
pub trait HolonStagingBehavior {
    /// Does a lookup by key on staged holons. Note HolonTypes are not required to offer a "key"
    fn get_staged_holon_by_key(&self, key: &MapString) -> Result<StagedReference, HolonError>;

    /// Returns a count of the number of holons being staged
    fn staged_count(&self) -> i64;

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError>;

}
