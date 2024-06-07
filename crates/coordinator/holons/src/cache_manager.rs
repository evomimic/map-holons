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
    /// This method returns a sharable reference to the HolonCache for the specified holon_space_id.
    /// If holon_space_id is None, return reference to the local HolonCache.
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

    /// fetch_holon gets a specific HolonNode from the persistent store based on its ActionHash, it then
    /// "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon(id: &HolonId) -> Result<Holon, HolonError> {
        let holon_node_record = get(id.0.clone(), GetOptions::default())?;
        if let Some(node) = holon_node_record {
            let mut holon = Holon::try_from_node(node)?;
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

        info!("-------ENTERED: get_rc_holon, getting cache");
        let cache = self.get_cache(holon_space_id)?;

        {
            // Check if the holon is already in the cache
            debug!("borrowing the cache from the cache_manager");
            let try_cache_borrow = cache.try_borrow();
            match try_cache_borrow {
                Ok(cache) => {
                    debug!("checking the cache for holon_id: {:#?}", holon_id.clone());
                    if let Some(holon) = cache.0.get(holon_id) {
                        // Return the holon if found in the cache
                        return Ok(holon.clone());
                    }
                }
                Err(_e) => {
                    return Err(HolonError::FailedToBorrow("Unable borrow holon cache immutably".to_string()));
                }
            }
        }

        info!("holon not cached, fetching holon");
        let fetched_holon = Self::fetch_holon(holon_id)?;
        debug!("holon fetched");


        debug!("getting mutable reference to cache");
        let try_cache_mut = cache.try_borrow_mut();
        match try_cache_mut {
            Ok(mut cache) => {
                debug!("inserting fetched holon in the cache for holon_id: {:#?}", holon_id.clone());
                cache.0.insert(holon_id.clone(), Rc::new(fetched_holon.clone()));
                // Return the holon if found in the cache
                Ok(Rc::new(fetched_holon))

            }
            Err(_e) => {
                return Err(HolonError::FailedToBorrow("Unable borrow_mut holon cache".to_string()));
            }
        }


    }
}
