pub fn dev_mode_enabled() -> bool {
    match std::env::var("MAP_START_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "dev"),
        Err(_) => false,
    }
}
