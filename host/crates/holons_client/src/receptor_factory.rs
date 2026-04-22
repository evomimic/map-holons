use super::receptor_cache::{ReceptorCache, ReceptorKey};
use crate::{LocalRecoveryReceptor, Receptor};
use client_shared_types::{
    base_receptor::{BaseReceptor, ReceptorType},
    holon_space::SpaceInfo,
};
use core_types::HolonError;
use deprecated_holochain_receptor::HolochainReceptor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct ReceptorFactory {
    cache: ReceptorCache,
    is_loaded: Arc<AtomicBool>,
}

impl ReceptorFactory {
    pub fn new() -> Self {
        Self { cache: ReceptorCache::new(), is_loaded: Arc::new(AtomicBool::new(false)) }
    }

    /// Check if receptors have been loaded
    pub fn are_receptors_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }

    /// Create receptor from base configuration
    async fn create_receptor_from_base(
        &self,
        base: BaseReceptor,
    ) -> Result<Arc<Receptor>, Box<dyn std::error::Error>> {
        match base.receptor_type {
            ReceptorType::Local => Err(Box::new(HolonError::NotImplemented(
                "LocalReceptor creation is currently disabled".into(),
            ))),
            ReceptorType::LocalRecovery => {
                tracing::info!("Creating LocalRecoveryReceptor from base configuration");
                Ok(Arc::new(Receptor::LocalRecovery(LocalRecoveryReceptor::new(base)?)))
            }
            ReceptorType::Holochain => {
                tracing::info!("Creating HolochainReceptor from base configuration");
                Ok(Arc::new(Receptor::Holochain(HolochainReceptor::new(base))))
            }
        }
    }

    pub fn get_receptors_by_type(
        &self,
        receptor_type: &ReceptorType,
    ) -> Result<Vec<Arc<Receptor>>, HolonError> {
        let receptors = self.cache.get_by_type(*receptor_type)?;
        if receptors.is_empty() {
            return Err(HolonError::HolonNotFound(format!(
                "No receptors found for type: {:?}",
                receptor_type
            )));
        }
        Ok(receptors)
    }

    pub fn get_default_receptor_by_type(
        &self,
        receptor_type: &ReceptorType,
    ) -> Result<Arc<Receptor>, HolonError> {
        self.get_receptors_by_type(receptor_type)?.first().cloned().ok_or_else(|| {
            HolonError::HolonNotFound(format!(
                "No default receptor found for type: {:?}",
                receptor_type
            ))
        })
    }

    pub fn get_receptor_by_id(&self, receptor_id: &String) -> Result<Arc<Receptor>, HolonError> {
        let receptors = self.cache.get_by_id(receptor_id)?;
        if receptors.is_empty() {
            return Err(HolonError::HolonNotFound(format!(
                "No receptors found for id: {:?}",
                receptor_id
            )));
        }
        receptors.first().cloned().ok_or_else(|| {
            HolonError::HolonNotFound(format!("No receptors found for id: {:?}", receptor_id))
        })
    }

    //function returns all spaces in the root space
    pub fn get_root_spaces() -> Result<SpaceInfo, HolonError> {
        Err(HolonError::NotImplemented(
            "get_root_spaces to return all spaces in the root space".into(),
        ))
    }

    /// Clear cache (internal use only)
    pub(crate) fn _clear_cache(&self) -> Result<(), HolonError> {
        self.cache.clear()
    }

    /// Load receptors directly from a list of ReceptorConfig
    pub async fn load_from_configs(&self, configs: Vec<BaseReceptor>) -> Result<usize, HolonError> {
        // Clear existing cache
        self.cache.clear()?;
        self.is_loaded.store(false, Ordering::SeqCst);
        let mut count = 0;
        for cfg in configs {
            // Precompute key and cache the config
            let key = ReceptorKey::new(cfg.receptor_type, cfg.receptor_id.clone());
            // Cache the config for conductor lookup later
            match self.create_receptor_from_base(cfg).await {
                Ok(receptor) => {
                    self.cache.insert(key, receptor)?;
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

    pub async fn all_spaces_by_type(
        &self,
        rec_type: &ReceptorType,
    ) -> Result<SpaceInfo, HolonError> {
        for receptor in self.cache.get_by_type(*rec_type)? {
            return receptor.get_space_info().await;
        }
        Err(HolonError::NotImplemented("No conductor found for space".into()))
    }
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
