//! Defines the `ConductorDanceCaller` trait, which abstracts over different dance execution environments.

use holons_core::dances::{DanceRequest, DanceResponse};

pub trait ConductorDanceCaller {
    /// Sends a `DanceRequest` and returns a `DanceResponse`.
    ///
    /// Implementations of this trait define how dance requests are made based on
    /// the environment (native Holochain conductor, JavaScript bridge, or mock testing).
    ///
    /// This function is **synchronous** to ensure compatibility across different execution models.
    fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse;
}
