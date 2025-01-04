use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use crate::shared_objects_layer::api::HolonsContextFactory;
use crate::shared_objects_layer::implementation::context::HolonsContext;
use crate::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A concrete implementation of the `HolonsContextFactory` trait.
pub struct ConcreteHolonsContextFactory;

impl ConcreteHolonsContextFactory {
    /// Creates a new `ConcreteHolonsContextFactory` instance.
    pub fn new() -> Self {
        Self
    }
}

impl HolonsContextFactory for ConcreteHolonsContextFactory {
    /// Initializes a `HolonsContext` from session state information.
    ///
    /// # Arguments
    /// * `staged_holons` - A vector of staged holons wrapped in `Rc<RefCell>`.
    /// * `keyed_index` - A map of keys to their corresponding indices in the staged holons.
    /// * `local_space_holon` - An optional reference to the local space holon.
    ///
    /// # Returns
    /// A `Result` containing the initialized `HolonsContext` as a `Box<dyn HolonsContextBehavior>` or an error.
    fn init_context_from_session(
        &self,
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        local_space_holon: Option<HolonReference>,
    ) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
        // Step 1: Initialize the HolonsContext
        let context = HolonsContext::init_context_from_session(
            staged_holons,
            keyed_index,
            local_space_holon,
        )?;

        // Step 2: Return the context as a boxed trait object
        Ok(Box::new(context))
    }
}
