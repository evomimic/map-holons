use std::{cell::RefCell, fmt::Debug, rc::Rc};

use crate::core_shared_objects::Holon;
use crate::{HolonCollection, HolonsContextBehavior, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

pub trait HolonCacheAccess: Debug {
    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError>;

    fn related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;

    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError>;
}
