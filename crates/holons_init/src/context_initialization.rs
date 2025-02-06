//! This module initializes a `HolonsContext` based on session data.
//!
//! It abstracts the creation and initialization logic for a `HolonsContext` instance,
//! supporting distinct implementations for client-side and guest-side contexts
//! based on the compilation target.

#[cfg(not(feature = "client"))]
use crate::guest_context_factory::GuestHolonsContextFactory;

use crate::holons_context_factory::HolonsContextFactory;

use holons_core::core_shared_objects::{Holon, HolonError};
use holons_core::reference_layer::{HolonReference, HolonsContextBehavior};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[cfg(feature = "client")]
use crate::client_context_factory::ClientHolonsContextFactory;

/// Initializes the HolonsContext based on the execution environment (guest or client).
pub fn init_context_from_session(
    staged_holons: Vec<Rc<RefCell<Holon>>>,
    keyed_index: BTreeMap<MapString, usize>,
    local_space_holon: Option<HolonReference>,
) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
    // Select the appropriate factory based on the environment
    #[cfg(not(feature = "client"))]
    let factory = GuestHolonsContextFactory;

    #[cfg(feature = "client")]
    let factory = ClientHolonsContextFactory;

    // Delegate to the selected factory
    factory.init_context_with_staged_holons(staged_holons, keyed_index, local_space_holon)
}
