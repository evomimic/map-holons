//! Defines the `ConductorDanceCaller` trait, which abstracts over different dance execution environments.

use async_trait::async_trait;
use holons_core::dances::{DanceRequest, DanceResponse};
//use async_trait::async_trait;


#[async_trait(?Send)]
pub trait ConductorDanceCaller {
    /// Sends a `DanceRequest` and returns a `DanceResponse`.
    ///
    /// Implementations of this trait define how dance requests are made based on
    /// the environment (native Holochain conductor, JavaScript bridge, or mock testing).
    ///
    /// This function is **synchronous** to ensure compatibility across different execution models.
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse;
}
