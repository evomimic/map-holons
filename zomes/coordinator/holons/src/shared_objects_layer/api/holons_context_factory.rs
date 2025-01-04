use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use crate::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub trait HolonsContextFactory {
    /// Initializes a new HolonsContext based on the provided session data and ensures the
    /// space manager includes:
    /// - an initialized Nursery containing the staged holons and index provided,
    /// - an initialized local cache,
    /// - a local_holon_reference to the HolonSpace holon for this space. This may require
    /// retrieving the holon from the persistent store or creating it if it hasn't already been
    /// created
    ///
    /// This function takes the following parameters (assumed to be pulled from SessionState):
    ///
    /// - `staged_holons`: A vector of staged holons being built for commit.
    /// - `keyed_index`: A mapping of keys to staged holon indices for quick lookup.
    /// - `local_space_holon`: An optional reference to the local holon space, if already available.
    ///
    /// It returns a boxed instance of `HolonsContextBehavior` or an error.
    ///
    fn init_context_from_session(
        &self,
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        local_space_holon: Option<HolonReference>,
    ) -> Result<Box<dyn HolonsContextBehavior>, HolonError>;
}
