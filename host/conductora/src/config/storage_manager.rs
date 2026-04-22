use crate::config::providers::{
    holochain::{HolochainConfig, HolochainSelector},
    ipfs::IpfsConfig,
    local::LocalConfig,
    MultiEntrySelector, ProviderRuntimeSelection,
};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageManager {
    #[serde(default)]
    pub window_provider: Option<String>,
    #[serde(deserialize_with = "deserialize_unique_storage_providers")]
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

impl StorageManager {
    //discover and load the storage config file
    pub fn load_storage_config() -> anyhow::Result<StorageManager> {
        if let Ok(config_path) = std::env::var("STORAGE_CONFIG_PATH") {
            return Self::from_file(&config_path); // hard fail — they asked for a specific file
        }

        let config_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/storage.json");

        if config_path.exists() {
            return Self::from_file(config_path.to_str().unwrap());
        }

        Err(anyhow::anyhow!(
            "No storage config found. Expected at '{}' or set STORAGE_CONFIG_PATH env var.",
            config_path.display()
        ))
    }

    /// Load configuration from file
    fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: StorageManager = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Get a storage provider entry (name + config) by its name
    pub fn get_provider_entry(&self, name: &str) -> Option<(&String, &StorageProvider)> {
        self.storage_providers.get_key_value(name)
    }

    /// Resolve provider types for runtime-selected provider keys.
    ///
    /// Returns:
    /// - deduped, deterministic provider types
    /// - warning messages for any missing provider keys
    pub fn runtime_provider_types(
        &self,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> (Vec<&'static str>, Vec<String>) {
        let (runtime_providers, warnings) = self.runtime_provider_entries(runtime_selection);
        let mut provider_types: BTreeSet<&'static str> = BTreeSet::new();

        for (_, provider) in runtime_providers {
            provider_types.insert(provider.provider_type());
        }

        (provider_types.into_iter().collect(), warnings)
    }

    /// Resolve runtime provider entries for selected runtime provider keys.
    ///
    /// Returns:
    /// - resolved runtime provider entries as owned `(provider_key, provider)` tuples
    /// - warning messages for any missing provider keys
    pub fn runtime_provider_entries(
        &self,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> (Vec<(String, StorageProvider)>, Vec<String>) {
        let mut runtime_providers = Vec::new();
        let mut warnings = Vec::new();

        for provider_key in &runtime_selection.runtime_provider_keys {
            match self.get_provider_entry(provider_key) {
                Some((name, provider)) => {
                    runtime_providers.push((name.clone(), provider.clone()));
                }
                None => {
                    warnings.push(format!(
                        "Runtime selection references missing provider '{}'",
                        provider_key
                    ));
                }
            }
        }

        (runtime_providers, warnings)
    }

    /// Resolve runtime provider keys, window provider key, and selection warnings.
    pub fn resolve_runtime_selection(&self) -> anyhow::Result<ProviderRuntimeSelection> {
        let holochain_entries: Vec<(&str, &HolochainConfig)> = self
            .storage_providers
            .iter()
            .filter_map(|(key, provider)| match provider {
                StorageProvider::Holochain(cfg) if cfg.enabled => Some((key.as_str(), cfg)),
                _ => None,
            })
            .collect();

        let selected_holochain_keys = HolochainSelector::select_keys(&holochain_entries);
        let mut warnings =
            HolochainSelector::warnings(&holochain_entries, &selected_holochain_keys);

        let mut runtime_provider_keys: Vec<String> = self
            .storage_providers
            .iter()
            .filter(|(_, provider)| {
                provider.is_enabled()
                    && provider.provider_type() != HolochainSelector::PROVIDER_TYPE
            })
            .map(|(key, _)| key.clone())
            .collect();

        runtime_provider_keys.extend(selected_holochain_keys.clone());
        runtime_provider_keys.sort();
        runtime_provider_keys.dedup();

        let window_provider_key = match self.window_provider.as_deref() {
            Some(alias) if HolochainSelector::window_aliases().contains(&alias) => {
                selected_holochain_keys.first().cloned()
            }
            Some(name) => {
                let (_, provider) = self.get_provider_entry(name).ok_or_else(|| {
                    anyhow::anyhow!("window_provider '{}' not found in providers", name)
                })?;
                if !provider.is_enabled() {
                    return Err(anyhow::anyhow!("window_provider '{}' is disabled", name));
                }

                if provider.provider_type() == HolochainSelector::PROVIDER_TYPE {
                    let active = selected_holochain_keys.first().ok_or_else(|| {
                        anyhow::anyhow!(
                            "window_provider '{}' points to holochain, but no enabled holochain provider is available",
                            name
                        )
                    })?;

                    if active != name {
                        return Err(anyhow::anyhow!(
                            "window_provider '{}' is not the active holochain provider '{}'",
                            name,
                            active
                        ));
                    }
                }

                Some(name.to_string())
            }
            None => selected_holochain_keys.first().cloned(),
        };

        warnings.sort();

        Ok(ProviderRuntimeSelection { runtime_provider_keys, window_provider_key, warnings })
    }
}

impl StorageProvider {
    pub fn is_enabled(&self) -> bool {
        match self {
            StorageProvider::Holochain(c) => c.enabled,
            StorageProvider::Ipfs(c) => c.enabled,
            StorageProvider::Local(c) => c.enabled,
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

fn deserialize_unique_storage_providers<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, StorageProvider>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UniqueStorageProvidersVisitor;

    impl<'de> Visitor<'de> for UniqueStorageProvidersVisitor {
        type Value = HashMap<String, StorageProvider>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a map of uniquely named storage providers")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut providers = HashMap::new();

            while let Some((key, value)) = map.next_entry::<String, StorageProvider>()? {
                if providers.insert(key.clone(), value).is_some() {
                    return Err(de::Error::custom(format!(
                        "duplicate storage provider key '{}' in storage_providers",
                        key
                    )));
                }
            }

            Ok(providers)
        }
    }

    deserializer.deserialize_map(UniqueStorageProvidersVisitor)
}
