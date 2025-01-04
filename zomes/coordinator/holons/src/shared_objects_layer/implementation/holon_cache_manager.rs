use crate::shared_objects_layer::implementation::holon_cache::HolonCache;
use crate::shared_objects_layer::{Holon, HolonError, HolonResolver};
use hdi::prelude::debug;
use shared_types_holon::LocalId;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct HolonCacheManager {
    cache: RefCell<HolonCache>, // Use RefCell for interior mutability
    resolver: Arc<dyn HolonResolver>,
}

impl HolonCacheManager {
    /// Creates a new `HolonCacheManager` with the provided `HolonResolver`.
    pub fn new(resolver: Arc<dyn HolonResolver>) -> Self {
        Self {
            cache: RefCell::new(HolonCache::new()), // Wrap the cache in a RefCell
            resolver,
        }
    }

    /// Retrieves a local Holon by its `LocalId`.
    /// - If the Holon is already in the cache, it returns the cached reference.
    /// - Otherwise, it fetches the Holon using the resolver, adds it to the cache, and returns it.
    pub fn get_local_holon(&self, local_id: &LocalId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        // Borrow the cache immutably to check for an existing Holon
        if let Some(cached_holon) = self.cache.borrow().get(local_id) {
            debug!("Holon with LocalId {:?} retrieved from cache.", local_id);
            return Ok(cached_holon.clone());
        }

        // If not in cache, resolve the Holon using the resolver
        debug!("Holon with LocalId {:?} not in cache. Fetching using resolver.", local_id);
        let holon = self.resolver.fetch_holon(local_id)?;

        // Wrap the resolved Holon in a Rc<RefCell> for shared mutability
        let rc_holon = Rc::new(RefCell::new(holon));

        // Insert the Holon into the cache using a mutable borrow
        self.cache.borrow_mut().insert(local_id.clone(), rc_holon.clone());

        debug!("Holon with LocalId {:?} fetched and cached.", local_id);
        Ok(rc_holon)
    }
}
