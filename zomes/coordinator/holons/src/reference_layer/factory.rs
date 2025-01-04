//! This module provides access to the `HolonsContextFactory`.
//!
//! The factory encapsulates the initialization logic for creating `HolonsContext` instances,
//! ensuring a clean boundary between the `reference_layer` and lower layers like
//! `shared_objects_layer`. Higher layers, such as the `dance_layer`, can use this factory
//! to initialize context objects without directly depending on the implementation details
//! of the `shared_objects_layer`.

use crate::reference_layer::{HolonReference, HolonsContextBehavior};
pub use crate::shared_objects_layer::HolonsContextFactory;
use crate::shared_objects_layer::{ConcreteHolonsContextFactory, Holon, HolonError};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub fn init_context_from_session(
    staged_holons: Vec<Rc<RefCell<Holon>>>,
    keyed_index: BTreeMap<MapString, usize>,
    local_space_holon: Option<HolonReference>,
) -> Result<Box<dyn HolonsContextBehavior>, HolonError> {
    let factory = ConcreteHolonsContextFactory::new();
    factory.init_context_from_session(staged_holons, keyed_index, local_space_holon)
}
