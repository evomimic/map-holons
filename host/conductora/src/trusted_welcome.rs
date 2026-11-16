//! Packaged Welcome admission. Constitution stays disabled until its authorization contract exists.
use serde::Serialize;
use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const WINDOW_LABEL: &str = "welcome";
pub const PAGE: &str = "welcome.html";

/// The supported packaged path cannot opt back into development privileges through environment.
pub fn validate_packaged_configuration() -> anyhow::Result<()> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        anyhow::bail!("Packaged Welcome currently supports macOS on Apple Silicon only");
    }
    anyhow::ensure!(
        !tauri::is_dev(),
        "Packaged Welcome requires the custom-protocol build feature"
    );
    for name in ["MAP_START_MODE", "STORAGE_CONFIG_PATH", "MAP_APPLICATION_EXPERIENCE"] {
        if std::env::var_os(name).is_some() {
            anyhow::bail!(
                "{name} is a development override and is not supported by packaged Welcome"
            );
        }
    }
    Ok(())
}

fn approved_page(label: &str, url: &tauri::Url) -> bool {
    label == WINDOW_LABEL
        && url.scheme() == "tauri"
        && url.host_str() == Some("localhost")
        && url.path() == "/welcome.html"
        && url.query().is_none()
        && url.fragment().is_none()
}

fn admit(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|e| e.to_string())?;
    if crate::env::development_enabled() || !approved_page(window.label(), &url) {
        return Err("Request did not originate from packaged Welcome".into());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct WelcomeStatus {
    pub constitution_available: bool,
}

#[tauri::command]
pub fn welcome_status(window: WebviewWindow) -> Result<WelcomeStatus, String> {
    admit(&window)?;
    Ok(WelcomeStatus { constitution_available: false })
}

#[tauri::command]
pub fn constitute_personal_space(window: WebviewWindow) -> Result<(), String> {
    admit(&window)?;
    // No supplied ID, bootstrap flag or transport credential can enable this operation.
    Err("Personal space setup is not available in this build".into())
}

pub fn create_window(handle: &tauri::AppHandle) -> anyhow::Result<()> {
    WebviewWindowBuilder::new(handle, WINDOW_LABEL, WebviewUrl::App(PAGE.into()))
        .title("Welcome to MAP")
        .inner_size(720.0, 540.0)
        .on_navigation(|url| approved_page(WINDOW_LABEL, url))
        .build()?;
    Ok(())
}

/// No conductor credentials or runtime references are exposed by this boundary.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn welcome_permissions_exclude_conductor_and_generic_map_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/welcome.json")).unwrap();
        assert_eq!(capability["webviews"], serde_json::json!(["welcome"]));
        assert!(capability.get("windows").is_none());
        assert!(capability.get("remote").is_none());
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["allow-welcome-status", "allow-constitute-personal-space"])
        );
        let page = include_str!("../../ui/public/welcome.html");
        assert!(page.contains("script-src 'self';"));
        assert!(page.contains("frame-src 'none';"));
        assert!(!page.contains("blob:"));
    }

    #[test]
    fn only_exact_packaged_welcome_page_is_admitted() {
        let trusted = tauri::Url::parse("tauri://localhost/welcome.html").unwrap();
        assert!(approved_page("welcome", &trusted));
        assert!(!approved_page("main", &trusted));
        assert!(!approved_page("welcome-untrusted-content", &trusted));
        for url in [
            "http://localhost:4200/welcome.html",
            "https://example.org/welcome.html",
            "tauri://localhost/index.html",
            "tauri://localhost/welcome.html?bootstrap=true",
            "blob:tauri://localhost/code",
        ] {
            assert!(!approved_page("welcome", &tauri::Url::parse(url).unwrap()), "{url}");
        }
    }
}
