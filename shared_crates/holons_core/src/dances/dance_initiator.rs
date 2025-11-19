use async_trait::async_trait;

use crate::dances::{DanceRequest, DanceResponse};
use crate::HolonsContextBehavior;
use std::fmt::Debug;

/// Canonical trait for initiating outbound Dances.
///
/// This trait forms the outer boundary of the Trust Channel stack,
/// abstracting over the environment-specific mechanism that transmits
/// a [`DanceRequest`] and returns a [`DanceResponse`].
/// Implementations may delegate to a Holochain conductor,
/// a Tauri bridge, or other runtime adapters.

/// Production trait: requires Send + Sync for multi-threaded contexts
#[async_trait]
pub trait DanceInitiator: Send + Sync + Debug {
    /// Sends a `DanceRequest` and returns a `DanceResponse`.
    ///
    /// Implementations define how Dances are initiated based on
    /// the environment (e.g., native conductor, Tauri bridge, or mock testing).
    async fn initiate_dance(
        &self,
        context: &dyn HolonsContextBehavior,
        request: DanceRequest,
    ) -> DanceResponse;
}
