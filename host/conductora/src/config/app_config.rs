use super::storage_config::StorageConfig;

//pub const APP_ID: &'static str = env!("APP_ID");

/// Load storage configuration from file or create default - should only be called once
pub fn load_storage_config() -> StorageConfig {
    let dev_mode = hc_dev_mode_enabled();

    // Try environment variable first
    if let Ok(config_path) = std::env::var("STORAGE_CONFIG_PATH") {
        tracing::debug!("[CONFIG] Loading from STORAGE_CONFIG_PATH: {}", config_path);
        let mut cfg = StorageConfig::from_file(&config_path).unwrap_or_else(|e| {
            tracing::warn!("[CONFIG] Failed to load from {}: {}. Using default.", config_path, e);
            StorageConfig::default()
        });
        apply_holochain_mode_selection(&mut cfg, dev_mode);
        return cfg;
    }

    // Get path relative to this file's directory
    let config_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/config");
    let config_path = config_dir.join("storage.json");

    tracing::debug!("[CONFIG] Looking for config at: {:?}", config_path);
    
    if config_path.exists() {
        let mut cfg = StorageConfig::from_file(config_path.to_str().unwrap()).unwrap_or_else(|e| {
            tracing::warn!("[CONFIG] Failed to load from {:?}: {}. Using default.", config_path, e);
            StorageConfig::default()
        });
        apply_holochain_mode_selection(&mut cfg, dev_mode);
        return cfg;
    }

    tracing::warn!("[CONFIG] File not found at: {:?}. Using default.", config_path);
    let mut cfg = StorageConfig::default();
    apply_holochain_mode_selection(&mut cfg, dev_mode);
    cfg
}

pub fn hc_dev_mode_enabled() -> bool {
    match std::env::var("HC_DEV_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn apply_holochain_mode_selection(cfg: &mut StorageConfig, dev_mode: bool) {
    if let Err(e) = cfg.select_holochain_profile(dev_mode) {
        tracing::error!("[CONFIG] {}", e);
        panic!("Storage configuration error: {}", e);
    }
}
