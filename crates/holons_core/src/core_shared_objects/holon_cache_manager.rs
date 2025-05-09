use crate::core_shared_objects::holon_cache::HolonCache;

use crate::core_shared_objects::{
    Holon, HolonCacheAccess, HolonCollection, HolonError, RelationshipCache, RelationshipName,
};
use crate::reference_layer::HolonServiceApi;
use hdk::prelude::debug;
use core_types::HolonId;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug)]
pub struct HolonCacheManager {
    cache: RefCell<HolonCache>, // Use RefCell for interior mutability
    relationship_cache: RefCell<RelationshipCache>,
    holon_service: Arc<dyn HolonServiceApi>,
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(holon_service: Arc<dyn HolonServiceApi>) -> Self {
        Self {
            cache: RefCell::new(HolonCache::new()), // Wrap the cache in a RefCell
            holon_service,
            relationship_cache: RefCell::new(RelationshipCache::new()),
        }
    }
}

impl HolonCacheAccess for HolonCacheManager {
    /// Retrieves a Holon by its `HolonId`.
    /// - If the Holon is already in the cache, it returns the cached reference.
    /// - Otherwise, it fetches the Holon using the HolonService, adds it to the cache, and returns it.
    /// The behavior of this method is different on the client-side, where all Holons are cached
    /// in a single cache, and the guest-side where each space has its own cache.
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        // Borrow the cache immutably to check for an existing Holon
        if let Some(cached_holon) = self.cache.borrow().get(holon_id) {
            debug!("Holon with HolonId {:?} retrieved from cache.", holon_id);
            return Ok(cached_holon.clone());
        }

        // If not in cache, resolve the Holon using the resolver
        debug!("Holon with HolonId {:?} not in cache. Fetching using HolonSpace.", holon_id);
        let holon = self.holon_service.fetch_holon(holon_id)?;

        // Wrap the resolved Holon in a Rc<RefCell> for shared mutability
        let rc_holon = Rc::new(RefCell::new(holon));

        // Insert the Holon into the cache using a mutable borrow
        self.cache.borrow_mut().insert(holon_id.clone(), rc_holon.clone());

        debug!("Holon with LocalId {:?} fetched and cached.", holon_id);
        Ok(rc_holon)
    }

    fn get_related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        self.relationship_cache.borrow().get_related_holons(
            self.holon_service.as_ref(),
            source_holon_id,
            relationship_name,
        )
    }
}
