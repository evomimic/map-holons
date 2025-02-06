use holons_core::core_shared_objects::{Holon, HolonError};
use holons_core::reference_layer::{HolonReference, HolonsContextBehavior};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

// Define the trait here
pub trait HolonsContextFactory {
    fn init_context_with_staged_holons(
        &self,
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        local_space_holon: Option<HolonReference>,
    ) -> Result<Box<dyn HolonsContextBehavior>, HolonError>;
}
