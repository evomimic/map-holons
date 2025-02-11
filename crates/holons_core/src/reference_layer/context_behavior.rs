use crate::reference_layer::HolonSpaceBehavior;
use std::sync::Arc;

pub trait HolonsContextBehavior {
    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Arc<dyn HolonSpaceBehavior>;
}
