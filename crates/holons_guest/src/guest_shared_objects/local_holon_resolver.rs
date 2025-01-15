use holons_core::{Holon, HolonError, HolonResolver};

use crate::holon_service;
use shared_types_holon::HolonId;

/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
#[derive(Debug, Clone)]
pub struct LocalHolonResolver;

impl HolonResolver for LocalHolonResolver {
    /// Fetches a Holon by its `HolonId` by delegating to the `holon_service`.
    ///
    /// # Arguments
    ///
    /// * `holon_id` - The `HolonId` of the Holon to fetch.
    ///
    /// # Returns
    ///
    /// A `Result` containing the fetched `Holon` or a `HolonError`.
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        holon_service::fetch_holon(holon_id)
    }
}
