use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use client_shared_types::base_receptor::ReceptorType;
use core_types::HolonError;

use crate::Receptor;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReceptorKey {
    pub receptor_type: ReceptorType,
    pub receptor_id: String,
}

impl ReceptorKey {
    pub fn new(receptor_type: ReceptorType, receptor_id: String) -> Self {
        Self { receptor_type, receptor_id }
    }
}

// Updated to be thread-safe
#[derive(Clone)]
pub struct ReceptorCache {
    cache: Arc<Mutex<HashMap<ReceptorKey, Arc<Receptor>>>>,
}

impl ReceptorCache {
    pub fn new() -> Self {
        Self { cache: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn lock_cache(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<ReceptorKey, Arc<Receptor>>>, HolonError> {
        self.cache.lock().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Receptor cache lock poisoned: {e}"))
        })
    }

    pub fn get(&self, key: &ReceptorKey) -> Result<Option<Arc<Receptor>>, HolonError> {
        Ok(self.lock_cache()?.get(key).cloned())
    }

    pub fn get_by_id(&self, receptor_id: &String) -> Result<Vec<Arc<Receptor>>, HolonError> {
        let cache = self.lock_cache()?;
        Ok(cache
            .iter()
            .filter_map(
                |(key, receptor)| {
                    if key.receptor_id == *receptor_id {
                        Some(receptor.clone())
                    } else {
                        None
                    }
                },
            )
            .collect())
    }

    pub fn get_by_type(
        &self,
        receptor_type: ReceptorType,
    ) -> Result<Vec<Arc<Receptor>>, HolonError> {
        let cache = self.lock_cache()?;
        Ok(cache
            .iter()
            .filter_map(|(key, receptor)| {
                if key.receptor_type == receptor_type {
                    Some(receptor.clone())
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn insert(&self, key: ReceptorKey, receptor: Arc<Receptor>) -> Result<(), HolonError> {
        let mut cache = self.lock_cache()?;
        cache.insert(key, receptor);
        tracing::debug!("Cached receptor. Total cached: {}", cache.len());
        Ok(())
    }

    pub fn remove(&self, key: &ReceptorKey) -> Result<Option<Arc<Receptor>>, HolonError> {
        Ok(self.lock_cache()?.remove(key))
    }

    pub fn clear(&self) -> Result<(), HolonError> {
        self.lock_cache()?.clear();
        tracing::debug!("Receptor cache cleared");
        Ok(())
    }

    pub fn len(&self) -> Result<usize, HolonError> {
        Ok(self.lock_cache()?.len())
    }

    pub fn is_empty(&self) -> Result<bool, HolonError> {
        Ok(self.lock_cache()?.is_empty())
    }

    // probably remove, Check if a receptor exists for the given space
    // pub fn has_receptor_for_space(&self, space_id: &String) -> bool {
    //     let key = ReceptorKey::from_space_holon(space_id);
    //      self.lock_cache()?.contains_key(&key)
    //  }
}

// Custom Debug implementation since Mutex doesn't derive Debug easily
impl std::fmt::Debug for ReceptorCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cache_size = match self.cache.lock() {
            Ok(cache) => cache.len(),
            Err(_) => 0,
        };
        f.debug_struct("ReceptorCache").field("cache_size", &cache_size).finish()
    }
}

impl Default for ReceptorCache {
    fn default() -> Self {
        Self::new()
    }
}
