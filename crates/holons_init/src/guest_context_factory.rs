use crate::holons_context_factory::HolonsContextFactory;

use holons_core::core_shared_objects::{Holon, HolonError};
use holons_core::reference_layer::{HolonReference, HolonsContextBehavior};
use holons_guest::guest_context::guest_context::GuestHolonsContext;
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A guest-side service object used to create a new HolonsContext
pub struct GuestHolonsContextFactory;

// impl GuestHolonsContextFactory {
//     /// Creates a new `GuestHolonsContextFactory` instance.
//     pub fn new() -> Self {
//         Self
//     }
// }

impl HolonsContextFactory for GuestHolonsContextFactory {
    /// Initializes a new HolonsContext based on the provided session data and ensures the
    /// space manager includes:
    /// - a guest_holon_service object
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
    fn init_context_with_staged_holons(
        &self,
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        local_space_holon: Option<HolonReference>,
    ) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
        // Step 1: Initialize the HolonsContext
        let context = GuestHolonsContext::init_context_from_session(
            staged_holons,
            keyed_index,
            local_space_holon,
        )?;

        // Step 2: Return the context as a boxed trait object
        Ok(Box::new(context))
    }
}
