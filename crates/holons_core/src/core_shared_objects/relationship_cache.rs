use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tracing::debug;

use crate::core_shared_objects::{HolonCollection, RelationshipMap, RelationshipName};
use crate::reference_layer::HolonServiceApi;
use core_types::{HolonError, HolonId};


#[derive(Clone, Debug)]
pub struct RelationshipCache {
    cache: Rc<RefCell<HashMap<HolonId, RelationshipMap>>>,
}

impl RelationshipCache {
    /// Creates a new RelationshipCache with an empty cache.
    pub fn new() -> Self {
        Self { cache: Rc::new(RefCell::new(HashMap::new())) }
    }

    /// Retrieves the `HolonCollection` containing references to all holons that are related
    /// to the specified `source_holon_id` via the specified `relationship_name` Note
    /// that the `HolonCollection` could be empty.
    ///
    /// If the relationship data for the `source_holon_id` and `relationship_name` is already in
    /// the cache, it is returned immediately. Otherwise, it is fetched from the `HolonServiceApi`,
    /// inserted into the cache, and then returned.
    ///
    /// This implementation supports both **lazy loading** and **exactly-once** semantics. The first
    /// time a cache miss occurs for a `source_holon_id`, an entry for that `source_holon_id` is added
    /// to the cache, along with an entry for the requested `relationship_name`. If there are no
    /// target holons for the requested relationship, an empty `HolonCollection` is cached, avoiding
    /// repeated calls to the `fetch_related_holons` method of the `HolonServiceApi`.
    pub fn get_related_holons(
        &self,
        holon_service: &dyn HolonServiceApi,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // First, check if the relationship exists in the cache (immutable borrow)
        {
            let cache = self.cache.borrow();
            if let Some(relationship_map) = cache.get(&source_holon_id) {
                if let Some(related_holons) =
                    relationship_map.get_collection_for_relationship(relationship_name)
                {
                    // Cache hit: return the cached HolonCollection
                    debug!(
                        "Cache hit for source_holon_id: {:?}, relationship_name: {:?}",
                        source_holon_id, relationship_name
                    );
                    return Ok(related_holons.clone());
                }
            }
        } // Immutable borrow ends here

        // Cache miss: Fetch related holons from the HolonServiceApi
        debug!(
        "Cache miss for source_holon_id: {:?}, relationship_name: {:?}. Fetching from HolonServiceApi.",
        source_holon_id, relationship_name
    );
        let fetched_holons =
            holon_service.fetch_related_holons(&source_holon_id, relationship_name)?;

        // Wrap the fetched holons in an Rc
        let fetched_holons_rc = Rc::new(fetched_holons);

        // Update the cache (mutable borrow only for this scope)
        {
            let mut cache = self.cache.borrow_mut();
            let relationship_map =
                cache.entry(source_holon_id.clone()).or_insert_with(RelationshipMap::new_empty);
            relationship_map.insert(relationship_name.clone(), fetched_holons_rc.clone());
        } // Mutable borrow ends here

        // Return the fetched holons
        Ok(fetched_holons_rc)
    }
    /// Returns a RelationshipMap containing entries for all populated relationships from the given
    /// source_id.
    ///
    /// _*NOTE: Without the source_id's HolonDescriptor, there is no way of knowing what
    /// relationships originate from that source_id. Thus, this request cannot be satisfied from
    /// the relationship_cache and must always be delegated to the holon service. The results,
    /// however, ARE added to the cache.*_
    ///
    pub fn get_all_populated_relationships(
        &self,
        _holon_service: &dyn HolonServiceApi,
        _source_holon_id: HolonId,
    ) -> Result<Rc<RelationshipMap>, HolonError> {
        todo!()
    }
}
