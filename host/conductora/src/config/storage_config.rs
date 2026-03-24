use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::config::providers::holochain::HolochainConfig;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub default_storage: Option<String>,
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

        // Default Holochain profiles (mode-selected via HC_DEV_MODE)
        let mut dev_holochain = HolochainConfig::default();
        dev_holochain.network_seed = Some("dev_seed1234".to_string());
        dev_holochain.bootstrap_url = Some("http://0.0.0.0:8888".to_string());
        dev_holochain.signal_url = None;
        dev_holochain.enabled = false;
        storage_providers.insert(
            "holochain_dev".to_string(),
            StorageProvider::Holochain(dev_holochain),
        );

        let mut prod_holochain = HolochainConfig::default();
        prod_holochain.network_seed = Some("prod_seed1234".to_string());
        prod_holochain.bootstrap_url = Some("https://bootstrap.holo.host".to_string());
        prod_holochain.signal_url = Some("wss://sbd.holo.host".to_string());
        prod_holochain.enabled = false;
        storage_providers.insert(
            "holochain_production".to_string(),
            StorageProvider::Holochain(prod_holochain),
        );

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
            default_storage: None,
            storage_providers,
        }
    }

    pub fn get_enabled_provider_by_type(&self, provider_type: &str) -> Option<&StorageProvider> {
        self.storage_providers
            .values()
            .find(|provider| provider.is_enabled() && provider.provider_type() == provider_type)
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
        // Check default storage only when provided.
        if let Some(default_storage) = &self.default_storage {
            if !self.storage_providers.contains_key(default_storage) {
                return Err(format!(
                    "Default storage '{}' not found in providers",
                    default_storage
                ));
            }
        }

        // At least one provider must be enabled
        if !self.get_enabled_providers().is_empty() {
            Ok(())
        } else {
            Err("At least one storage provider must be enabled".to_string())
        }
    }

    /// Select the active holochain profile based on startup mode.
    ///
    /// `dev_mode=true` -> `holochain_dev`
    /// `dev_mode=false` -> `holochain_production`
    ///
    /// Fails fast when the required profile is missing or has the wrong type.
    pub fn select_holochain_profile(&mut self, dev_mode: bool) -> Result<(), String> {
        let selected_key = if dev_mode {
            "holochain_dev"
        } else {
            "holochain_production"
        };

        let selected_provider = self
            .storage_providers
            .get(selected_key)
            .ok_or_else(|| {
                format!(
                    "Missing required profile '{}'. \
                     Ensure storage.json defines both 'holochain_dev' and 'holochain_production'.",
                    selected_key
                )
            })?;

        if !matches!(selected_provider, StorageProvider::Holochain(_)) {
            return Err(format!(
                "Profile '{}' must be type='holochain' to match HC_DEV_MODE selection.",
                selected_key
            ));
        }

        // Keep both profiles in config, but activate only the selected one.
        for (name, provider) in self.storage_providers.iter_mut() {
            if let StorageProvider::Holochain(cfg) = provider {
                cfg.enabled = name == selected_key;
            }
        }

        self.default_storage = None;
        Ok(())
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
