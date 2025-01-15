use crate::reference_layer::{HolonReference, HolonsContextBehavior};
// use crate::shared_objects_layer::api::HolonsContextFactory;
use holons::shared_objects_layer::implementation::context::HolonsContext;
use holons::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A concrete implementation of the `HolonsContextFactory` trait.
pub struct GuestHolonsContextFactory;

impl GuestHolonsContextFactory {
    /// Creates a new `ConcreteHolonsContextFactory` instance.
    pub fn new() -> Self {
        Self
    }
}

impl GuestHolonsContextFactory {
    /// Initializes a new HolonsContext based on the provided session data and ensures the
    /// space manager includes:
    /// - an initialized Nursery containing the staged holons and index provided,
    /// - an initialized local cache,
    /// - a local_holon_reference to the HolonSpace holon for this space. This may require
    /// retrieving the holon from the persistent store or creating it if it hasn't already been
    /// created.
    ///
    /// # Arguments
    /// * `staged_holons` - A vector of staged holons wrapped in `Rc<RefCell>`.
    /// * `keyed_index` - A map of keys to their corresponding indices in the staged holons.
    /// * `local_space_holon` - An optional reference to the local space holon.
    ///
    /// # Returns
    /// A `Result` containing the initialized `HolonsContext` as a `Box<dyn HolonsContextBehavior>` or an error.
    pub fn init_context_from_session(
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
