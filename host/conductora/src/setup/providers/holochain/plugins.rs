use crate::config::providers::holochain::HolochainConfig;
use std::path::PathBuf;
use tauri_plugin_holochain::{vec_to_locked, HolochainPluginConfig, NetworkConfig};

/// Configure Holochain plugin
pub fn holochain_plugin(
    provider_key: &str,
    hc_cfg: &HolochainConfig,
) -> Result<impl tauri::plugin::Plugin<tauri::Wry>, anyhow::Error> {
    let mut plugin_config = HolochainPluginConfig::new(
        holochain_dir(hc_cfg)?,
        network_config_from_storage_config(hc_cfg),
    )
    .signal_url_configured(hc_cfg.signal_url.is_some());
    if hc_dev_mode_enabled() {
        let dir = dev_conductor_dir(provider_key, &hc_cfg.app_id)?;
        plugin_config = plugin_config.dev_mode().dev_data_root(dir);
    }
    Ok(tauri_plugin_holochain::async_init(vec_to_locked(vec![]), plugin_config))
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

pub fn holochain_dir(hc_cfg: &HolochainConfig) -> Result<PathBuf, anyhow::Error> {
    if tauri::is_dev() {
        let tmp_dir =
            tempdir::TempDir::new(&hc_cfg.app_id).map_err(|e| anyhow::anyhow!("Could not create temporary directory: {}", e))?;
        // Convert `tmp_dir` into a `Path`, destroying the `TempDir` without deleting the directory.
        Ok(tmp_dir.into_path())
    } else {
        let app_name: &'static str = Box::leak(hc_cfg.app_id.clone().into_boxed_str());
        let path = app_dirs2::app_root(
            app_dirs2::AppDataType::UserData,
            &app_dirs2::AppInfo { name: app_name, author: env!("CARGO_PKG_AUTHORS") },
        )
        .expect("Could not get app root")
        .join("holochain");
        Ok(path)    
    }
}

// Generate a deterministic dev conductor directory path based on provider name, app id, and network seed.
// this serves as the data_root for a conductor in dev mode,
// allowing us to preserve the WASM cache across restarts while still having a unique directory per provider/app/seed.
// Why a hardcoded absolute path (not std::env::temp_dir()):
// Inside Nix shells TMPDIR is a session-specific directory like
// /tmp/nix-shell.1TXdRd/ that changes on every new shell invocation.
// Using temp_dir() would give a different path each run, losing the
// WASM compile cache.  /tmp is always available on macOS/Linux.
pub fn dev_conductor_dir(provider_name: &str, app_id: &str) -> Result<std::path::PathBuf, anyhow::Error> {
    let workspace = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Failed to resolve current directory: {}", e))?
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to canonicalize current directory: {}", e))?;
    let input = format!("{provider_name}:{app_id}:{}", workspace.display());

    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut h);
    let key = format!("{:016x}", h.finish());

    let root = std::path::PathBuf::from("/tmp/conductora_dev");
    std::fs::create_dir_all(&root)
        .map_err(|e| anyhow::anyhow!("Failed to create dev conductor root {:?}: {}", root, e))?;

    Ok(root.join(key))
}

pub fn hc_dev_mode_enabled() -> bool {
    match std::env::var("HC_DEV_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}
