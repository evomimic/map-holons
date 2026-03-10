use crate::config::providers::holochain::HolochainConfig;
use crate::config::providers::ipfs::IpfsConfig;
use crate::config::providers::local::LocalConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub default_storage: String,
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
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: StorageConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Get a storage provider by its name
    pub fn get_provider(&self, name: &str) -> Option<&StorageProvider> {
        self.storage_providers.get(name)
    }

    /// Get all enabled storage providers
    pub fn get_enabled_providers(&self) -> Vec<(&String, &StorageProvider)> {
        self.storage_providers.iter().filter(|(_, provider)| provider.is_enabled()).collect()
    }

    /// Validate the configuration
    pub fn _validate(&self) -> Result<(), String> {
        // Check if default storage exists
        if !self.storage_providers.contains_key(&self.default_storage) {
            return Err(format!(
                "Default storage '{}' not found in providers",
                self.default_storage
            ));
        }

        // At least one provider must be enabled
        if !self.get_enabled_providers().is_empty() {
            Ok(())
        } else {
            Err("At least one storage provider must be enabled".to_string())
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
