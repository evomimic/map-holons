use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;

use crate::dances::{DanceRequest, DanceResponse};
use crate::HolonsContextBehavior;

/// Trait-object façade for DanceCallService so SpaceManager can store it
/// without knowing the concrete conductor backend type.
#[async_trait(?Send)]
pub trait DanceCallServiceApi: Debug + Any {
    fn as_any(&self) -> &dyn Any;

    /// Executes a dance call with session state management.
    async fn dance_call(
        &self,
        context: &dyn HolonsContextBehavior,
        request: DanceRequest,
    ) -> DanceResponse;
}
