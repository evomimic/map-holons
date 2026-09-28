use super::Holon;
use core_types::{HolonError, HolonId};
use quick_cache::sync::Cache;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct HolonCache {
    entries: Arc<Cache<HolonId, Arc<RwLock<Holon>>>>,
    retained: Arc<RwLock<HashMap<HolonId, (usize, Arc<RwLock<Holon>>)>>>,
}

/// Keeps one saved cache entry resident without changing reference resolution semantics.
/// Shared leases retain the same content; dropping the last lease restores ordinary eviction.
pub struct SavedHolonRetention {
    cache: HolonCache,
    id: HolonId,
}
impl Drop for SavedHolonRetention {
    fn drop(&mut self) {
        if let Ok(mut retained) = self.cache.retained.write() {
            if let Some((count, _)) = retained.get_mut(&self.id) {
                *count -= 1;
                if *count == 0 {
                    retained.remove(&self.id);
                }
            }
        }
    }
}

impl HolonCache {
    /// Creates a new HolonCache with a default size.
    pub fn new() -> Self {
        Self::new_with_capacity(99) // Default size
    }

    /// Creates a new HolonCache with a custom size.
    ///
    /// # Arguments
    ///
    /// * `size` - The desired capacity of the cache.
    #[allow(dead_code)]
    pub fn new_with_capacity(size: usize) -> Self {
        Self {
            entries: Arc::new(Cache::new(size)),
            retained: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /// Retains an already resolved entry in the cache's residency table.
    pub(crate) fn retain(
        &self,
        id: HolonId,
        value: Arc<RwLock<Holon>>,
    ) -> Result<SavedHolonRetention, HolonError> {
        let mut retained =
            self.retained.write().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        let entry = retained.entry(id.clone()).or_insert((0, value));
        entry.0 += 1;
        Ok(SavedHolonRetention { cache: self.clone(), id })
    }

    /// Retrieves a reference to a cached item by key.
    pub fn get(&self, key: &HolonId) -> Option<Arc<RwLock<Holon>>> {
        if let Ok(retained) = self.retained.read() {
            if let Some((_, value)) = retained.get(key) {
                return Some(Arc::clone(value));
            }
        }
        self.entries.get(key)
    }
    /// Inserts an item into the cache.
    pub fn insert(&self, key: HolonId, value: Arc<RwLock<Holon>>) {
        self.entries.insert(key, value);
    }
}
impl Deref for HolonCache {
    type Target = Cache<HolonId, Arc<RwLock<Holon>>>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
//impl DerefMut for HolonCache {
//   fn deref_mut(&mut self) -> &mut Self::Target {
//      &mut self.0
//  }
//}
