use super::storage_config::StorageConfig;

/// Load storage configuration from file. Returns an error if config cannot be loaded — application cannot start without it.
pub fn load_storage_config() -> Result<StorageConfig, Box<dyn std::error::Error>> {
    if let Ok(config_path) = std::env::var("STORAGE_CONFIG_PATH") {
        return StorageConfig::from_file(&config_path); // hard fail — they asked for a specific file
    }

    let config_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/storage.json");

    if config_path.exists() {
        return StorageConfig::from_file(config_path.to_str().unwrap());
    }

    Err(format!(
        "No storage config found. Expected at '{:?}' or set STORAGE_CONFIG_PATH env var.",
        config_path
    )
    .into())
}
