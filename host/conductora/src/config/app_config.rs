use super::storage_config::StorageConfig;

//pub const APP_ID: &'static str = env!("APP_ID");

/// Load storage configuration from file or create default - should only be called once
pub fn load_storage_config() -> StorageConfig {
    // Try environment variable first
    if let Ok(config_path) = std::env::var("STORAGE_CONFIG_PATH") {
        tracing::debug!("[CONFIG] Loading from STORAGE_CONFIG_PATH: {}", config_path);
        return StorageConfig::from_file(&config_path).unwrap_or_else(|e| {
            tracing::warn!("[CONFIG] Failed to load from {}: {}. Using default.", config_path, e);
            StorageConfig::default()
        });
    }

    // Get path relative to this file's directory
    let config_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/config");
    let config_path = config_dir.join("storage.json");

    tracing::debug!("[CONFIG] Looking for config at: {:?}", config_path);
    
    if config_path.exists() {
        return StorageConfig::from_file(config_path.to_str().unwrap()).unwrap_or_else(|e| {
            tracing::warn!("[CONFIG] Failed to load from {:?}: {}. Using default.", config_path, e);
            StorageConfig::default()
        });
    }

    tracing::warn!("[CONFIG] File not found at: {:?}. Using default.", config_path);
    StorageConfig::default()
}