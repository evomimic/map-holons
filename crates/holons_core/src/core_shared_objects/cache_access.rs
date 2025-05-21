use super::holon::Holon;
use crate::{HolonCollection, HolonError, RelationshipName};
use shared_types_holon::HolonId;
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

pub trait HolonCacheAccess: Debug {
    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError>;

    fn get_related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;
}
