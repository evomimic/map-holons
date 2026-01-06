use async_trait::async_trait;
use holons_core::dances::{DanceRequest, DanceResponse};
use std::fmt::Debug;

/// Native-only interface for executing a DanceRequest against a Holochain conductor.
///
/// This trait is implemented by host-side components that have privileged access
/// to a running conductor, such as:
///   • `HolochainConductorClient` (production)
///   • protocol-specific receptors (future)
///   • test doubles (integration tests)
///
/// IMPORTANT:
///   - This trait is *not* WASM-safe.
///   - It is intentionally kept in native-only shared_crates.
///   - The returned future must be `Send` because native receptors operate inside
///     multithreaded Tokio runtimes.
///
/// This is the host → conductor execution primitive, which is then wrapped by
/// higher-level abstractions (TrustChannel, DanceInitiator).
#[async_trait]
pub trait ConductorDanceCaller: Debug + Send + Sync {
    /// Execute a single-shot DanceRequest and return the DanceResponse produced by the conductor.
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse;
}
