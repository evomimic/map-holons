use crate::config::providers::holochain::HolochainConfig;
use crate::config::providers::ipfs::IpfsConfig;
use crate::config::providers::local::LocalConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub default_storage: String,
    #[serde(default)]
    pub window_provider: Option<String>,
    pub storage_providers: HashMap<String, StorageProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StorageProvider {
    #[serde(rename = "holochain")]
    Holochain(HolochainConfig),

    #[serde(rename = "ipfs")]
    Ipfs(IpfsConfig),

    #[serde(rename = "local")]
    Local(LocalConfig),
}

impl StorageConfig {

    //discover and load the storage config file
    pub fn load_storage_config() -> Result<StorageConfig, Box<dyn std::error::Error>> {
        if let Ok(config_path) = std::env::var("STORAGE_CONFIG_PATH") {
            return Self::from_file(&config_path); // hard fail — they asked for a specific file
        }

        let config_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/storage.json");

        if config_path.exists() {
            return Self::from_file(config_path.to_str().unwrap());
        }

        Err(format!(
            "No storage config found. Expected at '{:?}' or set STORAGE_CONFIG_PATH env var.",
            config_path
        )
        .into())
    }

    /// Load configuration from file
    fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: StorageConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Get a storage provider by its name
    pub fn get_provider(&self, name: &str) -> Option<&StorageProvider> {
        self.storage_providers.get(name)
    }

    /// Get a storage provider entry (name + config) by its name
    pub fn get_provider_entry(&self, name: &str) -> Option<(&String, &StorageProvider)> {
        self.storage_providers.get_key_value(name)
    }

    /// Get all enabled storage providers
    pub fn get_enabled_providers(&self) -> Vec<(&String, &StorageProvider)> {
        self.storage_providers.iter().filter(|(_, provider)| provider.is_enabled()).collect()
    }

    /// Get enabled providers filtered by provider type
    pub fn get_enabled_providers_by_type(
        &self,
        provider_type: &str,
    ) -> Vec<(&String, &StorageProvider)> {
        self.get_enabled_providers()
            .into_iter()
            .filter(|(_, provider)| provider.provider_type() == provider_type)
            .collect()
    }

    /// TODO - this is a bit hacky since it assumes that only Holochain providers can be window providers. 
    /// We may want to revisit this logic as we add more provider types and window integration options.
    
    /// Resolve which provider should be used for window creation.
    /// If `window_provider` is set, it must exist and be enabled.
    /// Otherwise, if exactly one Holochain provider is enabled, use it.
    /// If multiple Holochain providers are enabled, require `window_provider`.
    pub fn resolve_window_provider(&self) -> Result<Option<(&String, &StorageProvider)>, String> {
        if let Some(name) = self.window_provider.as_deref() {
            let (key, provider) = self
                .get_provider_entry(name)
                .ok_or_else(|| format!("window_provider '{}' not found in providers", name))?;
            if !provider.is_enabled() {
                return Err(format!("window_provider '{}' is disabled", name));
            }
            return Ok(Some((key, provider)));
        }

        let enabled_holochain = self.get_enabled_providers_by_type("holochain");
        match enabled_holochain.len() {
            0 => Ok(None),
            1 => Ok(Some(enabled_holochain[0])),
            _ => {
                let names: Vec<&str> = enabled_holochain
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect();
                Err(format!(
                    "Multiple enabled Holochain providers {:?}; set window_provider to select one",
                    names
                ))
            }
        }
    }
}


/// Common interface for all provider config types.
pub trait ProviderConfig: serde::Serialize {
    fn is_enabled(&self) -> bool;
    /// Missing field (None) or false → no snapshot store created.
    fn snapshot_recovery(&self) -> bool {
        false
    }
}

impl StorageProvider {
    pub fn is_enabled(&self) -> bool {
        match self {
            StorageProvider::Holochain(c) => c.is_enabled(),
            StorageProvider::Ipfs(c) => c.is_enabled(),
            StorageProvider::Local(c) => c.is_enabled(),
        }
    }

    pub fn provider_type(&self) -> &'static str {
        match self {
            StorageProvider::Holochain(_) => "holochain",
            StorageProvider::Ipfs(_) => "ipfs",
            StorageProvider::Local(_) => "local",
        }
    }
}
