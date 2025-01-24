use crate::reference_layer::{HolonReference, HolonSpaceBehavior};
use std::rc::Rc;

use crate::core_shared_objects::HolonError;
use shared_types_holon::MapString;

pub trait HolonsContextBehavior {
    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Rc<&dyn HolonSpaceBehavior>;

    fn add_references_to_dance_state(&self, holons: Vec<HolonReference>) -> Result<(), HolonError>;
    fn add_reference_to_dance_state(&self, holon_ref: HolonReference) -> Result<(), HolonError>;

    fn get_by_key_from_dance_state(
        &self,
        key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError>;
}
