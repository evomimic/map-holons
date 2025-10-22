//! Defines the `ConductorDanceCaller` trait, which abstracts over different
//! dance execution environments (native conductor, WASM bridge, mocks).

use async_trait::async_trait;
use std::fmt::Debug;
use crate::dances::{DanceRequest, DanceResponse};

#[async_trait(?Send)]
pub trait ConductorDanceCaller: Debug {
    /// Sends a `DanceRequest` and returns a `DanceResponse`.
    ///
    /// Implementations of this trait define how dance requests are made based on
    /// the environment (native Holochain conductor, JavaScript bridge, or mock testing).
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse;
}