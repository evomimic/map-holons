use crate::reference_layer::HolonSpaceBehavior;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;

#[async_trait]
pub trait HolonsContextBehavior: Debug + Send + Sync {
    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior>;
}
