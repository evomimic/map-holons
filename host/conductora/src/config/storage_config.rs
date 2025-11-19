use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::config::providers::holochain::{HolochainConfig};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsConfig {
    pub api_url: String,
    pub gateway_url: String,
    pub repo_path: Option<PathBuf>,
    pub swarm_key: Option<String>,
    pub bootstrap_peers: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub data_dir: PathBuf,
    pub max_size_mb: Option<u64>,
    pub compression: bool,
    pub encryption: bool,
    pub enabled: bool,
}

impl StorageConfig {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: StorageConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Create default configuration
    pub fn default() -> Self {
        let mut storage_providers = HashMap::new();

        // Default Holochain provider
        storage_providers.insert("holochain".to_string(), StorageProvider::Holochain(HolochainConfig::default()));

        // Default IPFS provider
        storage_providers.insert(
            "ipfs_main".to_string(),
            StorageProvider::Ipfs(IpfsConfig {
                api_url: "http://127.0.0.1:5001".to_string(),
                gateway_url: "http://127.0.0.1:8080".to_string(),
                repo_path: None,
                swarm_key: None,
                bootstrap_peers: vec![
                    "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ".to_string()
                ],
                enabled: false, // Disabled by default
            })
        );

        // Default Local provider
        storage_providers.insert(
            "local_cache".to_string(),
            StorageProvider::Local(LocalConfig {
                data_dir: PathBuf::from("./data/local_storage"),
                max_size_mb: Some(1024), // 1GB limit
                compression: true,
                encryption: false,
                enabled: true,
            })
        );

        Self {
            default_storage: "holochain_main".to_string(),
            storage_providers,
        }
    }

    /// Get a storage provider by its name
    pub fn get_provider(&self, name: &str) -> Option<&StorageProvider> {
        self.storage_providers.get(name)
    }

    /// Get all enabled storage providers
    pub fn get_enabled_providers(&self) -> Vec<(&String, &StorageProvider)> {
        self.storage_providers
            .iter()
            .filter(|(_, provider)| provider.is_enabled())
            .collect()
    }

    /// Validate the configuration
    pub fn _validate(&self) -> Result<(), String> {
        // Check if default storage exists
        if !self.storage_providers.contains_key(&self.default_storage) {
            return Err(format!("Default storage '{}' not found in providers", self.default_storage));
        }

        // At least one provider must be enabled
        if !self.get_enabled_providers().is_empty() {
            Ok(())
        } else {
            Err("At least one storage provider must be enabled".to_string())
        }
    }
}

impl StorageProvider {
    pub fn is_enabled(&self) -> bool {
        match self {
            StorageProvider::Holochain(config) => config.enabled,
            StorageProvider::Ipfs(config) => config.enabled,
            StorageProvider::Local(config) => config.enabled,
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