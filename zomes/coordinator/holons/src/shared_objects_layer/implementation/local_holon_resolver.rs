use crate::shared_objects_layer::implementation::holon_service;
use crate::shared_objects_layer::{Holon, HolonError, HolonResolver};
use shared_types_holon::LocalId;

/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
#[derive(Debug, Clone)]
pub struct LocalHolonResolver;

impl HolonResolver for LocalHolonResolver {
    /// Fetches a Holon by its `LocalId` by delegating to the `holon_service`.
    ///
    /// # Arguments
    ///
    /// * `local_id` - The `LocalId` of the Holon to fetch.
    ///
    /// # Returns
    ///
    /// A `Result` containing the fetched `Holon` or a `HolonError`.
    fn fetch_holon(&self, local_id: &LocalId) -> Result<Holon, HolonError> {
        holon_service::get_holon_by_local_id(local_id)
    }
}
