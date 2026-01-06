use std::path::PathBuf;
// use map_commands::{StorageConfig, StorageProvider};
use serde::{Deserialize, Serialize};
use tauri_plugin_holochain::{vec_to_locked, AppBundle, HolochainPluginConfig, NetworkConfig};
use crate::config::{StorageConfig, StorageProvider};
//use crate::config::app_config::APP_ID;
// use tauri_plugin_holochain::{HolochainPluginConfig, vec_to_locked, NetworkConfig, AppBundle};
//use tauri_plugin_holochain::HolochainExt;

pub type CellDetails = Vec<CellDetail>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellDetail {
    pub role_name: String,
    pub zome_name: String,
    pub zome_function: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolochainConfig {
    pub network_seed: Option<String>,
    pub bootstrap_url: Option<String>,
    pub signal_url: Option<String>,
    pub proxy_url: Option<String>,
    pub target_arc_factor: Option<u32>,
    pub app_id: String,
    pub cell_details: Option<CellDetails>,
    pub happ_path: Option<String>, // Path to .happ file if not embedded
    pub enabled: bool,
}

//DON'T USE THIS! config is all in storage.json
impl Default for HolochainConfig {
    fn default() -> Self {
        HolochainConfig {
            network_seed: None,
            bootstrap_url: Some("http://0.0.0.0:8888".to_string()),
            signal_url: None,
            proxy_url: None,
            target_arc_factor: Some(1),
            app_id: "map_holons".to_string(),
            cell_details: None,
            happ_path: None,
            enabled: true,
        }
    }
}

    /// Configure Holochain plugin
pub fn holochain_plugin(provider: StorageProvider) ->  Result<impl tauri::plugin::Plugin<tauri::Wry>, anyhow::Error> {
    let StorageProvider::Holochain(hc_cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Holochain"));
    };
    Ok(tauri_plugin_holochain::async_init(
    vec_to_locked(vec![]),
    HolochainPluginConfig::new(holochain_dir(&hc_cfg), network_config_from_storage_config(&hc_cfg))
    ))
}

/// Load and validate the happ bundle from filesystem
pub fn load_happ_bundle(holochain_config: &HolochainConfig) -> Result<AppBundle, Box<dyn std::error::Error>> {
    // Get the path from HolochainConfig or use a sensible default
    let happ_relative = holochain_config.happ_path.clone().unwrap_or_else(|| {
        let default = "happ/workdir/map-holons.happ".to_string();
        tracing::warn!("[HAPP LOADER] ⚠️  happ_path not set in config, using default: {}", default);
        default
    });
    
    tracing::debug!("[HAPP LOADER] ✅ Using happ_path from config: {}", happ_relative);

    // Resolve relative to the workspace root, not current_dir
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Failed to determine workspace root")?;
    
    let happ_path = workspace_root.join(&happ_relative);
    
    tracing::debug!("[HAPP LOADER] Workspace root: {:?}", workspace_root);
    tracing::debug!("[HAPP LOADER] Loading happ from: {:?}", happ_path);
    tracing::debug!("[HAPP LOADER] Current directory: {:?}", std::env::current_dir());

    // Check if file exists
    if !happ_path.exists() {
        tracing::error!("[HAPP LOADER] ❌ File not found: {:?}", happ_path);
        return Err(format!("Happ file not found at: {:?}", happ_path).into());
    }
    
    tracing::debug!("[HAPP LOADER] ✅ File found");
    
    // Read file
    let bytes = std::fs::read(&happ_path)
        .map_err(|e| format!("Failed to read happ file: {}", e))?;
    
    tracing::debug!("[HOLOCHAIN SETUP] Happ file loaded successfully ({} bytes)", bytes.len());
    
    // Decode bundle
    let bundle = AppBundle::decode(&bytes)
        .map_err(|e| format!("Failed to decode happ bundle: {}", e))?;
    
    tracing::info!("[HOLOCHAIN SETUP] Happ bundle decoded successfully");
    Ok(bundle)
}


pub fn network_config_from_storage_config(holochain_config: &HolochainConfig) -> NetworkConfig {
    let mut network_config = NetworkConfig::default();

    // Use configuration from storage config
    if let Some(bootstrap_url) = &holochain_config.bootstrap_url {
        network_config.bootstrap_url = url2::Url2::parse(bootstrap_url);
    } else if tauri::is_dev() {
        // Fallback for dev mode
        network_config.bootstrap_url = url2::Url2::parse("http://0.0.0.0:8888");
    }

    if let Some(signal_url) = &holochain_config.signal_url {
        network_config.signal_url = url2::Url2::parse(signal_url);
    }

    //if let Some(proxy_url) = &holochain_config.proxy_url {
     //   network_config.proxy_url = Some(url2::Url2::parse(proxy_url));
    //}

    if let Some(target_arc_factor) = holochain_config.target_arc_factor {
        network_config.target_arc_factor = target_arc_factor;
    }

    // Don't hold any slice of the DHT in mobile
    if cfg!(mobile) {
        network_config.target_arc_factor = 0;
    }

    network_config
}

pub fn _network_config(storage_config: &StorageConfig) -> NetworkConfig {
    // Load storage config and extract Holochain settings
    //let storage_config = load_storage_config();
    
    // Find the first enabled Holochain provider
    for (_, provider) in storage_config.get_enabled_providers() {
        if let StorageProvider::Holochain(holochain_config) = provider {
            return network_config_from_storage_config(holochain_config);
        }
    }
    
    // Fallback to default if no Holochain provider found
    NetworkConfig::default()
}


pub fn holochain_dir(hc_cfg: &HolochainConfig) -> PathBuf {
    if tauri::is_dev() {
        let tmp_dir = tempdir::TempDir::new(&hc_cfg.app_id)
            .expect("Could not create temporary directory");
        
        // Convert `tmp_dir` into a `Path`, destroying the `TempDir`
        // without deleting the directory.
        tmp_dir.into_path()
    } else {
        let app_name: &'static str = Box::leak(hc_cfg.app_id.clone().into_boxed_str());
        app_dirs2::app_root(
            app_dirs2::AppDataType::UserData,
            &app_dirs2::AppInfo {
                name: app_name,
                author: env!("CARGO_PKG_AUTHORS"),
            },
        )
        .expect("Could not get app root")
        .join("holochain")
    }
}