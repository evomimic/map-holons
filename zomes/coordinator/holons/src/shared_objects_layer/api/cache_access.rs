use crate::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::HolonId;
use std::cell::RefCell;
use std::rc::Rc;

pub trait HolonCacheAccess {
    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError>;
}
