use crate::reference_layer::{HolonReference, HolonSpaceBehavior};
use std::rc::Rc;
use std::sync::Arc;

use crate::core_shared_objects::HolonError;
use shared_types_holon::MapString;

use super::HolonCollectionApi;

pub trait HolonsContextBehavior {
    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Rc<&dyn HolonSpaceBehavior>;
}
