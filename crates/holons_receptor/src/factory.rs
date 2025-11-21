//use crate::local_receptor;

use super::{
    local_receptor::LocalReceptor,
    cache::{ReceptorCache, ReceptorKey}
};
use core_types::HolonError;
use holochain_receptor::{HolochainReceptor};
use holons_client::shared_types::{holon_space::SpaceInfo, base_receptor::{BaseReceptor, Receptor as ReceptorTrait}};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use sha2::{Digest, Sha256};
use hex;

#[derive(Clone)]
pub struct ReceptorFactory {
    cache: ReceptorCache,
    is_loaded: Arc<AtomicBool>,
}

impl ReceptorFactory {
    pub fn new() -> Self {
        Self {
            cache: ReceptorCache::new(),
            is_loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if receptors have been loaded
    pub fn are_receptors_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }
    
    /// Create receptor from base configuration
    async fn create_receptor_from_base(&self, base: BaseReceptor) -> Result<Arc<dyn ReceptorTrait>, Box<dyn std::error::Error>> {
        match base.receptor_type.as_str() {
            "local" => { //local should always be created first
                tracing::info!("Creating LocalReceptor from base configuration");
                let receptor = LocalReceptor::new(base)?;
                Ok(Arc::new(receptor) as Arc<dyn ReceptorTrait>)
            }
            "holochain" => {
                tracing::info!("Creating HolochainReceptor from base configuration");
                let receptor = HolochainReceptor::new(base);
                //let local_receptor = self.get_receptor_by_type("local");
                //local_receptor.add_space()//.get_space_info().await?;
                //TODO: add home_space_holon to the local root_space
                Ok(Arc::new(receptor) as Arc<dyn ReceptorTrait>)
            }
            _ => Err(format!("Unsupported receptor type: {}", base.receptor_type).into())
        }
    }

    pub fn get_receptor_by_type(&self, receptor_type: &str) -> Arc<dyn ReceptorTrait> {
        let receptors = self.cache.get_by_type(receptor_type)
            .into_iter().collect::<Vec<_>>();
        if receptors.is_empty() {
            panic!("No receptors found for type: {}", receptor_type);
        }
        receptors[0].clone()
    }

    //function returns all spaces in the root space
    pub fn get_root_spaces() -> Result<SpaceInfo, HolonError> { 
        todo!("Implement get_root_spaces to return all spaces in the root space")
    }

    pub async fn load_holons(&self, _receptor_id: String, _holon_paths: Vec<String>) -> Result<(), HolonError> {
        todo!("Implement load_holons to load holons into receptors")
    }
  
    /// Clear cache (internal use only)
    pub(crate) fn _clear_cache(&self) {
        self.cache.clear();
    }

    /// Load receptors directly from a list of ReceptorConfig
    pub async fn load_from_configs(
        &self,
        configs: Vec<BaseReceptor>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        // Clear existing cache
        self.cache.clear();
        self.is_loaded.store(false, Ordering::SeqCst);
        let mut count = 0;
        for cfg in configs {
            // Precompute key and cache the config
            let id = generate_receptor_id(cfg.properties.clone())?;
            let key = ReceptorKey::new(cfg.receptor_type.clone(), id);
            // Cache the config for conductor lookup later
            match self.create_receptor_from_base(cfg).await {
                Ok(receptor) => {
                    self.cache.insert(key, receptor);
                    count += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to create receptor from config: {}", e);
                }
            }
        }
        self.is_loaded.store(true, Ordering::SeqCst);
        Ok(count)
    }


    pub async fn all_spaces_by_type(&self, rec_type: &str) -> Result<SpaceInfo, HolonError> {
       for receptor in self.cache.get_by_type(rec_type) {
           match rec_type {
               "holochain" => return receptor.get_space_info().await,
               _ => {}
           }
           //if let Some(conductor) = &cfg.conductor {
             //  let spaces = HolochainConductor::get_all_spaces(conductor.as_ref(), "map_holons")?;
             //  return Ok(spaces);
          // }
       }
       Err(HolonError::NotImplemented("No conductor found for space".into()))
    }

    // These methods only called for Holochain receptors
   /*  fn extract_cell_id_from_space(&self, space_id: &String) -> Result<CellId, Box<dyn std::error::Error>> {
        // Implementation remains the same
        todo!("Parse CellId from space holon property_map")
    }

    fn extract_zome_name_from_space(&self, space_id: &String) -> Result<String, Box<dyn std::error::Error>> {
        // Implementation remains the same
        Ok("holons".to_string())
    }
    */
}

//helpers
fn generate_receptor_id(props: HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
    let json = serde_json::to_string(&props)?;
    Ok(hex::encode(Sha256::digest(json.as_bytes())))
}

// Add Debug implementation
impl std::fmt::Debug for ReceptorFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReceptorFactory")
            .field("cache", &self.cache)
            .field("is_loaded", &self.is_loaded.load(Ordering::SeqCst))
            .finish()
    }
}
