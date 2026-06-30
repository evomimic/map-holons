pub fn hc_dev_mode_enabled() -> bool {
    match std::env::var("HC_DEV_MODE") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}
