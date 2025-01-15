//! This module initializes a `HolonsContext` based on session data.
//!
//! It abstracts the creation and initialization logic for a `HolonsContext` instance,
//! supporting distinct implementations for client-side and guest-side contexts
//! based on the compilation target.

use crate::reference_layer::{HolonReference, HolonsContextBehavior};

#[cfg(feature = "guest")]
use crate::shared_objects_layer::GuestHolonsContextFactory;
use crate::{Holon, HolonError};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

// #[cfg(feature = "client")]
// use crate::shared_objects_layer::implementation::client_holons_context_factory::ClientHolonsContextFactory;

/// Initializes the HolonsContext based on the execution environment (guest or client).
pub fn init_context_from_session(
    staged_holons: Vec<Rc<RefCell<Holon>>>,
    keyed_index: BTreeMap<MapString, usize>,
    local_space_holon: Option<HolonReference>,
) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
    #[cfg(feature = "guest")]
    {
        let factory = GuestHolonsContextFactory::new();
        factory.init_context_from_session(staged_holons, keyed_index, local_space_holon)
    }

    // #[cfg(feature = "client")]
    // {
    //     let factory = ClientHolonsContextFactory::new();
    //     factory.init_context_from_session(staged_holons, keyed_index, local_space_holon)
    // }
}
