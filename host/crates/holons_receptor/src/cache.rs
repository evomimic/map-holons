use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use holons_client::shared_types::base_receptor::Receptor as ReceptorTrait;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReceptorKey {
    pub receptor_type: String,
    pub receptor_id: String,
}

impl ReceptorKey {
    pub fn new(receptor_type: String, receptor_id: String) -> Self {
        Self {
            receptor_type,
            receptor_id,
        }
    }
}

// Updated to be thread-safe
#[derive(Clone)]
pub struct ReceptorCache {
    cache: Arc<Mutex<HashMap<ReceptorKey, Arc<dyn ReceptorTrait>>>>,
}

impl ReceptorCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &ReceptorKey) -> Option<Arc<dyn ReceptorTrait>> {
        self.cache.lock().unwrap().get(key).cloned()
    }

    pub fn get_by_type(&self, receptor_type: &str) -> Vec<Arc<dyn ReceptorTrait>> {
        let cache = self.cache.lock().unwrap();
        cache.iter()
            .filter_map(|(key, receptor)| {
                if key.receptor_type == receptor_type {
                    Some(receptor.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn insert(&self, key: ReceptorKey, receptor: Arc<dyn ReceptorTrait>) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(key, receptor);
        tracing::debug!("Cached receptor. Total cached: {}", cache.len());
    }

    pub fn remove(&self, key: &ReceptorKey) -> Option<Arc<dyn ReceptorTrait>> {
        self.cache.lock().unwrap().remove(key)
    }

    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
        tracing::debug!("Receptor cache cleared");
    }

    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }

    // probably remove, Check if a receptor exists for the given space
   // pub fn has_receptor_for_space(&self, space_id: &String) -> bool {
   //     let key = ReceptorKey::from_space_holon(space_id);
  //      self.cache.lock().unwrap().contains_key(&key)
  //  }
}

// Custom Debug implementation since Mutex doesn't derive Debug easily
impl std::fmt::Debug for ReceptorCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReceptorCache")
            .field("cache_size", &self.cache.lock().unwrap().len())
            .finish()
    }
}

impl Default for ReceptorCache {
    fn default() -> Self {
        Self::new()
    }

}