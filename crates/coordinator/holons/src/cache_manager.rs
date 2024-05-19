use crate::holon::Holon;
use crate::holon_error::HolonError;
use hdk::prelude::*;
use std::cell::RefCell;
// use hdk::prelude::*;
use quick_cache::unsync::Cache;
use shared_types_holon::HolonId;

use std::rc::Rc;
#[derive(Debug)]
pub struct HolonCache(Cache<HolonId, Rc<Holon>>);

#[derive(Debug)]
pub struct HolonCacheManager {
    pub local_cache: Rc<RefCell<HolonCache>>,
    //pub external_caches: HashMap<HolonSpaceId, HolonCache>,
}

impl HolonCacheManager {
    pub fn new() -> Self {
        // Initialize local cache
        let local_cache = Cache::new(99);

        // Wrap local_cache in a Rc<RefCell<_>>
        let local_cache_rc = Rc::new(RefCell::new(HolonCache(local_cache)));

        // Create and return a new HolonCacheManager
        HolonCacheManager {
            local_cache: local_cache_rc,
            //external_caches: HashMap::new(),
        }
    }
    pub fn get_cache(
        &self,
        holon_space_id: Option<&HolonId>,
    ) -> Result<Rc<RefCell<HolonCache>>, HolonError> {
        if let Some(_) = holon_space_id {
            return Err(HolonError::NotImplemented(
                "External HolonReference not implemented".to_string(),
            ));
        }
        // Return a reference to the local cache
        Ok(Rc::clone(&self.local_cache))
    }

    pub fn get_cache_mut(
        &mut self,
        holon_space_id: Option<&HolonId>,
    ) -> Result<Rc<RefCell<HolonCache>>, HolonError> {
        if let Some(_) = holon_space_id {
            return Err(HolonError::NotImplemented(
                "External HolonReference not implemented".to_string(),
            ));
        }
        // Return a mutable reference to the local cache
        Ok(Rc::clone(&self.local_cache))
    }

    /// fetch_holon gets a specific HolonNode from the persistent store based on its ActionHash, it then
    /// "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon(id: &HolonId) -> Result<Holon, HolonError> {
        let holon_node_record = get(id.0.clone(), GetOptions::default())?;
        if let Some(node) = holon_node_record {
            let holon = Holon::try_from_node(node)?;
            return Ok(holon);
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(id.0.to_string()))
        }
    }
    /// This method returns an immutable reference (Rc) to the Holon identified by holon_id within the cache
    /// associated with `holon_space_id` (or within the `local_cache` if `holon_space_id` is `None`).
    /// If the holon is not already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    ///
    /// TODO: Investigate whether quick_cache supports this behavior natively by passing in a closure to the
    /// fetch_holon function.
    ///

    pub fn get_rc_holon(
        &mut self,
        holon_space_id: Option<&HolonId>,
        holon_id: &HolonId,
    ) -> Result<Rc<Holon>, HolonError> {
        let cache = self.get_cache(holon_space_id)?;

        // Check if the holon is already in the cache
        let cache_borrow = cache.borrow();
        if let Some(holon) = cache_borrow.0.get(holon_id) {
            // Return the holon if found in the cache
            return Ok(holon.clone());
        }

        // If not found in the cache, fetch the holon
        let fetched_holon = Self::fetch_holon(holon_id)?;

        // Obtain a mutable reference to local_cache
        let cache_mut = self.get_cache_mut(holon_space_id)?;
        let mut cache_mut = cache_mut.borrow_mut();
        cache_mut
            .0
            .insert(holon_id.clone(), Rc::new(fetched_holon.clone()));

        // Return the fetched holon
        Ok(Rc::new(fetched_holon))
    }

    // pub fn get_rc_holon(
    //     &self,
    //     context: &HolonsContext,
    //     holon_space_id: Option<&HolonId>,
    //     holon_id: &HolonId,
    // ) -> Result<Rc<Holon>, HolonError> {
    //     let cache = self.get_cache(holon_space_id)?;
    //
    //     // Check if the holon is already in the cache
    //     let cache_borrow = cache.borrow();
    //     if let Some(holon) = cache_borrow.0.get(holon_id) {
    //         // Return the holon if found in the cache
    //         return Ok(holon.clone());
    //     }
    //
    //     // If not found in the cache, fetch the holon
    //     let fetched_holon = Self::fetch_holon(context, holon_id)?;
    //
    //     let mut self_mut = self;
    //     let mut cache_mut = self_mut.get_cache_mut(holon_space_id)?.borrow_mut();
    //     cache_mut.0.insert(holon_id.clone(), Rc::new(fetched_holon.clone()));
    //
    //
    //     // Return the fetched holon
    //     Ok(Rc::new(fetched_holon))
    // }
}
