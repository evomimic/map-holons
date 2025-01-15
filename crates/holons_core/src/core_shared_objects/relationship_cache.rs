use crate::{HolonCollection, HolonError, HolonServiceApi, RelationshipMap, RelationshipName};
use shared_types_holon::HolonId;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct RelationshipCache {
    store: Rc<RefCell<BTreeMap<HolonId, RelationshipMap>>>,
}

impl RelationshipCache {
    /// Creates a new HolonCache with a default size.
    pub fn new() -> Self {
        BTreeMap::new().into()
    }
    pub fn get_related_holons(
        &mut self,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        todo!()
        // Enhance `HolonCacheManager` to implement the `get_related_holons` trait function. This
        // implementation supports both _**lazy load**_ and **_exactly-once_** semantics. The first
        // time a cache miss occurs for a `source_id`, an entry for that `source_id` will be added
        // to the `RelationshipCache` and an entry for the requested `RelationshipName` will be
        // added to the `relationship_map` for that `source_id`, even if there are no target holons
        // for that relationship. Thus, repeated requests for that `relationship_name` for that
        // source holon's relationship can return the (empty)`HolonCollection` for that relationship
        // without triggering another `fetch_related_holons` call on its `HolonService`.
    }
}
