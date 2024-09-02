use crate::holon::Holon;
use crate::holon_error::HolonError;
use hdk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
// use hdk::prelude::*;
use quick_cache::unsync::Cache;
use shared_types_holon::{HolonId, HolonSpaceId, LocalId};

use std::rc::Rc;
use shared_types_holon::HolonId::{Local,External};

#[derive(Debug)]
pub struct HolonCache(Cache<HolonId, Rc<RefCell<Holon>>>);

#[derive(Debug)]
pub struct HolonCacheManager {
    pub local_cache: Rc<RefCell<HolonCache>>,
    // pub external_caches: HashMap<HolonSpaceId, HolonCache>,
    pub external_caches: HashMap<HolonSpaceId, Rc<RefCell<HolonCache>>>,
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
            external_caches: HashMap::new(),
        }
    }
    /// This method returns a sharable reference to the HolonCache for the specified HolonId.
    /// If HolonId is Local, return the local cache. Otherwise, extract the space_id from the
    /// External HolonId, and lookup the cache for that proxy in external_caches. Returns
    /// a HolonError::CacheError if this lookup fails.
        fn get_cache(
        &self,
        holon_id: &HolonId,
    ) -> Result<Rc<RefCell<HolonCache>>, HolonError> {

        match holon_id {
            Local(_) => {Ok(Rc::clone(&self.local_cache))}
            External(external_id) => {
                return if let Some(external_cache)
                    = self.external_caches.get(&external_id.space_id) {
                    Ok(Rc::clone(external_cache))
                } else {
                    Err(HolonError::CacheError("No cache found for this proxy_id".to_string()))
                }
            }
        }

    }

    /// fetch_local_holon gets a specific HolonNode from the local persistent store based on its ActionHash, it then
    /// "inflates" the HolonNode into a Holon and returns it
    fn fetch_local_holon(local_id: &LocalId) -> Result<Holon, HolonError> {
        let holon_node_record = get(local_id.0.clone(), GetOptions::default())?;
        if let Some(node) = holon_node_record {
            let holon = Holon::try_from_node(node)?;
            return Ok(holon);
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(local_id.0.to_string()))
        }
    }
    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    ///
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    /// TODO: Enhance to support `External` HolonIds
    ///

    pub fn get_rc_holon(
        &self,
        // holon_space_id: Option<&HolonId>,
        holon_id: &HolonId,
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        info!("-------ENTERED: get_rc_holon, getting cache");

        let cache = self.get_cache(holon_id)?;

        // Attempt to borrow the cache immutably
        {
            let try_cache_borrow = cache.try_borrow().map_err(|e| {
                HolonError::FailedToBorrow(format!("Unable to borrow holon cache immutably: {}", e))
            })?;

            // Check if the holon is already in the cache
            debug!("Checking the cache for local_id: {:#?}", holon_id.local_id());
            if let Some(holon) = try_cache_borrow.0.get(holon_id) {
                // Return a clone of the Rc<RefCell<Holon>> if found in the cache
                return Ok(Rc::clone(holon));
            }
        }

        // Holon not found in cache, fetch it
        info!("Holon not cached, fetching holon");

        let fetched_holon = match holon_id {
            Local(local_id) => {
                HolonCacheManager::fetch_local_holon(local_id)?
            }
            External(_) => {
                return Err(HolonError::NotImplemented("Fetch from external caches is not yet \
                implemented:".to_string()))
            }
        };
        debug!("Holon with key {:?} fetched", fetched_holon.get_key());

        // Attempt to borrow the cache mutably
        let mut cache_mut = cache.try_borrow_mut().map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow_mut holon cache: {}", e))
        })?;

        // Insert the fetched holon into the cache
        debug!(
            "Inserting fetched holon into cache for local_id: {:#?}",
            fetched_holon.get_local_id(),
        );
        cache_mut
            .0
            .insert(holon_id.clone(), Rc::new(RefCell::new(fetched_holon)));

        // Return a new Rc<RefCell<Holon>> containing the fetched holon
        Ok(Rc::clone(cache_mut.0.get(holon_id).expect("Holon should be present in the cache")))
    }

}
