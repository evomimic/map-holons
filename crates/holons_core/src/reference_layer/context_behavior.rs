use crate::reference_layer::HolonSpaceBehavior;
use std::fmt::Debug;
use std::sync::Arc;

pub trait HolonsContextBehavior: Debug {
    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior>;
}
