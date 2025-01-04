use crate::reference_layer::{HolonReference, HolonSpaceBehavior};
use std::rc::Rc;

use crate::shared_objects_layer::HolonError;
use shared_types_holon::MapString;

pub trait HolonsContextBehavior {
    /// Attempts to retrieve a clone of the Local Space Holon reference from the space manager.
    ///
    /// # Returns
    /// - `Some(HolonReference)` if the local space holon exists and is accessible.
    /// - `None` if no local space holon exists.
    fn get_local_space_holon(&self) -> Option<HolonReference>;

    /// Provides access to the query manager for executing queries on holons.
    //fn get_query_manager(&self) -> &dyn QueryManagerBehavior;

    // /// Provides access to the transient collection for managing request-specific state.
    // fn get_dance_state(&self) -> &dyn TransientCollectionBehavior;

    /// Provides access to the holon space manager for interacting with holons and their relationships.
    fn get_space_manager(&self) -> Rc<&dyn HolonSpaceBehavior>;

    fn add_references_to_dance_state(&self, holons: Vec<HolonReference>) -> Result<(), HolonError>;
    fn add_reference_to_dance_state(&self, holon_ref: HolonReference) -> Result<(), HolonError>;

    fn get_by_key_from_dance_state(
        &self,
        key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError>;
}
