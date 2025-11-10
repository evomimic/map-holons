//! Defines the `ConductorDanceCaller` trait, which abstracts over different
//! dance execution environments (native conductor, WASM bridge, mocks).

use async_trait::async_trait;
use std::fmt::Debug;
use crate::dances::{DanceRequest, DanceResponse};

#[async_trait(?Send)]
pub trait ConductorDanceCaller: Debug {
    /// Sends a `DanceRequest` and returns a `DanceResponse`.
    ///
    /// Implementations of this trait define how dance requests are executed in different
    /// environments (e.g., native Holochain conductor, JavaScript bridge, or mock testing).
    ///
    /// Although this method is asynchronous, each call represents a **single-shot**
    /// request/response interaction — it does not maintain a persistent or streaming session.
    ///
    /// The `?Send` bound allows implementations that operate in single-threaded contexts
    /// (such as WASM or JavaScript bridge environments) without requiring the returned
    /// future to be `Send`.
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse;
}
