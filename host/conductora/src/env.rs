pub fn dev_mode_enabled() -> bool {
    match std::env::var("MAP_START_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "dev"),
        Err(_) => false,
    }
}

/// Development privileges require both a debug build and Tauri's development mode.
/// A release binary without packaged assets must fail closed, never enable development ingress.
pub fn development_enabled() -> bool {
    cfg!(debug_assertions) && tauri::is_dev()
}
