use async_trait::async_trait;
use core_types::HolonError;
use holons_boundary::envelopes::{InternalDanceRequestEnvelope, InternalDanceResponseEnvelope};
use std::fmt::Debug;

/// Native-only interface for executing an internal dance envelope against a Holochain conductor.
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
/// higher-level abstractions (TrustChannel and envelope transport).
#[async_trait]
pub trait ConductorDanceCaller: Debug + Send + Sync {
    /// Execute a single-shot envelope and return the envelope produced by the conductor.
    async fn conductor_dance_call(
        &self,
        request: InternalDanceRequestEnvelope,
    ) -> Result<InternalDanceResponseEnvelope, HolonError>;
}
