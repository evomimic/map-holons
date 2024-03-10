use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_errors::HolonError;
use core::cell::RefCell;
use hdk::prelude::*;
use quick_cache::{sync::Cache, Equivalent};
use shared_types_holon::HolonId;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct HolonCacheManager {
    pub local_cache: HolonCache,
    // pub external_caches: HashMap<HolonSpaceId, HolonCache>,
}

#[derive(Debug, Clone)]
pub struct HolonCache {
    // fetcher: Dance, // the Dance that can be used to fetch records from the backing store for this cache
    pub map: HashMap<HolonId, CachedHolon>,
}

#[derive(Debug, Clone)]
pub struct CachedHolon {
    pub id: HolonId,
    pub til: Timestamp,
    pub holon: Holon,
}

impl HolonCacheManager {
    pub fn new() -> HolonCacheManager {
        HolonCacheManager {
            local_cache: HolonCache {
                map: HashMap::new(),
            },
        }
    }

    pub fn init_cache() -> Cache<HolonId, CachedHolon> {
        Cache::new(99)
    }

    pub fn update_cache(&mut self, holon_id: HolonId, cached_holon: CachedHolon) {
        self.local_cache.map.insert(holon_id, cached_holon);
    }

    pub fn get_rc_holon(
        context: &HolonsContext,
        holon_space_id: Option<HolonId>,
        holon_id: HolonId,
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        if let Some(id) = holon_space_id {
            return Holon::fetch_holon(context, id);
        } else {
            let map = context.clone().cache_manager.into_inner().local_cache.map;
            let cached_holon = map.get(&holon_id);
            if let Some(holon) = cached_holon {
                return Ok(Rc::new(RefCell::new(holon.holon.clone())));
            }
            Err(HolonError::HolonNotFound(
                "Invalid HolonId, Holon does not exist".to_string(),
            ))
        }
    }
}
