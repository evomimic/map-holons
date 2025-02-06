use crate::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use crate::core_shared_objects::{Holon, HolonError};
use std::any::Any;
use std::{cell::RefCell, rc::Rc};

/// A trait that defines access methods for managing and interacting with a nursery of staged holons.
/// NurseryAccess is a single-threaded trait for accessing nursery data.
/// It is not `Sync` or `Send` and must not be used in multi-threaded contexts.
pub trait NurseryAccess: Any {
    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon
    /// at the specified index into the StagedHolons vector
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>;

    /// Exposes internal functionality if supported.
    fn as_internal(&self) -> Rc<RefCell<dyn NurseryAccessInternal>>;
}
