// holons_core/src/dances/dance_call_service_api.rs
use crate::dances::{DanceRequest, DanceResponse};
use crate::HolonsContextBehavior;
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait(?Send)]
pub trait DanceCallServiceApi: Debug {
    async fn dance_call(&self, ctx: &dyn HolonsContextBehavior, req: DanceRequest)
        -> DanceResponse;
}
