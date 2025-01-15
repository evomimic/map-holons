use crate::reference_layer::StateMobility;
use crate::reference_layer::{HolonReference, HolonStagingBehavior};

use crate::HolonError;
use shared_types_holon::LocalId;
use std::any::Any;

pub trait HolonSpaceBehavior: HolonStagingBehavior + StateMobility {
    /// Included in this trait definition to allow downcasts so that some additional traits can be
    /// selectively exposed without making them visible to upper layers
    /// The Any trait allows checking and converting a reference to a concrete type at runtime.
    fn as_any(&self) -> &dyn Any; // Adds ability to convert to dyn Any
    /// Deletes a holon, ensuring all constraints (e.g., relationships) are respected.
    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError>;
    fn get_space_holon(&self) -> Option<HolonReference>;
}
