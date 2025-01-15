// use holons::shared_objects_layer::{Holon, HolonError, HolonResolver};
// use holons_guest::guest_persistence::holon_service;
// use shared_types_holon::LocalId;

use holons_core::{Holon, HolonError, HolonResolver};
use shared_types_holon::HolonId;

/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
#[derive(Debug, Clone)]
pub struct ClientHolonResolver;

impl HolonResolver for ClientHolonResolver {
    /// Fetches a Holon by its `HolonId` by delegating to the `fetch_holon` dance.
    ///
    /// # Arguments
    ///
    /// * `holon_id` - The `HolonId` of the Holon to fetch.
    ///
    /// # Returns
    ///
    /// A `Result` containing the fetched `Holon` or a `HolonError`.
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        todo!()
    }
}
