use crate::config::providers::holochain::HolochainConfig;
use std::path::PathBuf;
use tauri_plugin_holochain::{vec_to_locked, AppBundle, HolochainPluginConfig, NetworkConfig};

/// Configure Holochain plugin
pub fn holochain_plugin(
    provider_key: &str,
    hc_cfg: &HolochainConfig,
) -> Result<impl tauri::plugin::Plugin<tauri::Wry>, anyhow::Error> {
    let mut plugin_config = HolochainPluginConfig::new(
        holochain_dir(hc_cfg),
        network_config_from_storage_config(hc_cfg),
    )
    .signal_url_configured(hc_cfg.signal_url.is_some());
    if hc_dev_mode_enabled() {
        let dir = dev_conductor_dir(provider_key, &hc_cfg.app_id);
        plugin_config = plugin_config.dev_mode().dev_data_root(dir);
    }
    Ok(tauri_plugin_holochain::async_init(vec_to_locked(vec![]), plugin_config))
}

/// Load and validate the happ bundle from filesystem
pub fn load_happ_bundle(
    holochain_config: &HolochainConfig,
) -> Result<AppBundle, Box<dyn std::error::Error>> {
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
    let bytes =
        std::fs::read(&happ_path).map_err(|e| format!("Failed to read happ file: {}", e))?;

    tracing::debug!("[HOLOCHAIN SETUP] Happ file loaded successfully ({} bytes)", bytes.len());

    // Decode bundle
    let bundle =
        AppBundle::decode(&bytes).map_err(|e| format!("Failed to decode happ bundle: {}", e))?;

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

pub fn holochain_dir(hc_cfg: &HolochainConfig) -> PathBuf {
    if tauri::is_dev() {
        let tmp_dir =
            tempdir::TempDir::new(&hc_cfg.app_id).expect("Could not create temporary directory");

        // Convert `tmp_dir` into a `Path`, destroying the `TempDir`
        // without deleting the directory.
        tmp_dir.into_path()
    } else {
        let app_name: &'static str = Box::leak(hc_cfg.app_id.clone().into_boxed_str());
        app_dirs2::app_root(
            app_dirs2::AppDataType::UserData,
            &app_dirs2::AppInfo { name: app_name, author: env!("CARGO_PKG_AUTHORS") },
        )
        .expect("Could not get app root")
        .join("holochain")
    }
}

// Generate a deterministic dev conductor directory path based on provider name, app id, and network seed.
// this serves as the data_root for a conductor in dev mode,
// allowing us to preserve the WASM cache across restarts while still having a unique directory per provider/app/seed.
pub fn dev_conductor_dir(provider_name: &str, app_id: &str) -> std::path::PathBuf {
    let workspace = std::env::current_dir()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let input = format!("{provider_name}:{app_id}:{}", workspace.display());

    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut h);
    let key = format!("{:016x}", h.finish());

    std::path::PathBuf::from("/tmp/conductora_dev").join(key)
}

pub fn hc_dev_mode_enabled() -> bool {
    match std::env::var("HC_DEV_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}
