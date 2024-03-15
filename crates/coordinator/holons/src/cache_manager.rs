use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_errors::HolonError;
use core::cell::RefCell;
// use hdk::prelude::*;
use quick_cache::unsync::Cache;
use shared_types_holon::HolonId;

use std::rc::Rc;

#[derive(Debug)]
pub struct HolonCacheManager {
    pub local_cache: Rc<RefCell<Cache<HolonId, Rc<Holon>>>>,
    //pub external_caches: HashMap<HolonSpaceId, HolonCache>,
}

impl HolonCacheManager {
    pub fn new() -> HolonCacheManager {
        let cache: Cache<HolonId, Rc<Holon>> = Cache::new(99);
        HolonCacheManager {
            local_cache: Rc::new(RefCell::new(cache)),
        }
    }

    pub fn get_cache(
        &self,
        holon_space_id: Option<HolonId>,
    ) -> Result<Rc<RefCell<Cache<HolonId, Rc<Holon>>>>, HolonError> {
        return if let Some(_id) = holon_space_id {
            Err(HolonError::NotImplemented(
                "External HolonReference not implemented".to_string(),
            ))
        } else {
            Ok(Rc::clone(&self.local_cache))
        }
    }

    pub fn get_rc_holon(
        &self,
        context: &HolonsContext,
        holon_space_id: Option<HolonId>,
        holon_id: HolonId,
    ) -> Result<Rc<Holon>, HolonError> {
        return if let Some(_id) = holon_space_id {
            Err(HolonError::NotImplemented(
                "External HolonReference not implemented".to_string(),
            ))
        } else {
            let cache = self.get_cache(holon_space_id.clone())?;

            if let Some(holon) = cache.borrow_mut().get(&holon_id).clone() {
                return Ok(Rc::clone(holon));
            }

            let mut mut_cache = cache.borrow_mut();

            let fetched_holon = Holon::fetch_holon(context, holon_id.clone())?;
            mut_cache.insert(holon_id, fetched_holon.clone());
            Ok(fetched_holon)
        }
    }
}

// let get_mut_cache_result = self.get_cache(holon_space_id);
//                 if let Ok(mut_cache_ref) = get_mut_cache_result {
//                     let mut mut_cache = mut_cache_ref.borrow_mut();

//                     let fetched_holon = Holon::fetch_holon(context, holon_id.clone())?;
//                     mut_cache.insert(holon_id, fetched_holon.clone());
//                     return Ok(fetched_holon);
//                 } else {
//                     return Err(HolonError::CacheError(
//                         "Error getting local cache".to_string(),
//                     ));
//                 }
