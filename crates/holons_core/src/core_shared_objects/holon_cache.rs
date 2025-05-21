use super::holon::Holon;
use quick_cache::unsync::Cache;
use shared_types_holon::HolonId;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct HolonCache(Cache<HolonId, Rc<RefCell<Holon>>>);

impl HolonCache {
    /// Creates a new HolonCache with a default size.
    pub fn new() -> Self {
        HolonCache(Cache::new(99)) // Default size
    }

    /// Creates a new HolonCache with a custom size.
    ///
    /// # Arguments
    ///
    /// * `size` - The desired capacity of the cache.
    #[allow(dead_code)]
    pub fn new_with_capacity(size: usize) -> Self {
        HolonCache(Cache::new(size))
    }
    /// Retrieves a reference to a cached item by key.
    pub fn get(&self, key: &HolonId) -> Option<&Rc<RefCell<Holon>>> {
        self.0.get(key)
    }
    /// Inserts an item into the cache.
    pub fn insert(&mut self, key: HolonId, value: Rc<RefCell<Holon>>) {
        self.0.insert(key, value);
    }
}
impl Deref for HolonCache {
    type Target = Cache<HolonId, Rc<RefCell<Holon>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for HolonCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
