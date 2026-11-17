//! Packaged local Navigator window and admission over the existing application runtime.
use crate::{
    setup::application_launcher::{
        prepare_application_launch, ApplicationSessionPhase, ApplicationSessionState,
    },
    welcome_lifecycle::WelcomeLifecycle,
};
use map_commands_runtime::{
    application_launch::LaunchRequirement,
    personal_constitution::{PersonalConstitution, SovereigntyObservation},
};
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const WINDOW: &str = "navigator";
#[derive(Default)]
pub struct NavigatorLaunchLock(tokio::sync::Mutex<()>);

fn approved_page(label: &str, url: &tauri::Url) -> bool {
    label == WINDOW
        && url.scheme() == "tauri"
        && url.host_str() == Some("localhost")
        // Tauri omits index.html and leaves the custom-scheme origin path empty.
        && matches!(url.path(), "" | "/" | "/index.html" | "/load-holons")
        && url.query().is_none()
        && url.fragment().is_none()
}

/// Package identity plus a backend-established local acquired-space session.
/// This engineering profile does not claim production principal/delegation assurance.
pub(crate) fn admit_operational_window(window: &WebviewWindow) -> Result<(), String> {
    if crate::env::development_enabled() {
        return Ok(());
    }
    if !cfg!(all(feature = "local-constitution", debug_assertions))
        || !approved_page(window.label(), &window.url().map_err(|e| e.to_string())?)
    {
        return Err("Operational request did not originate from packaged Navigator".into());
    }
    let snapshot = window.app_handle().state::<ApplicationSessionState>().snapshot()?;
    if snapshot.phase != ApplicationSessionPhase::Ready
        || snapshot.active_agent.is_none()
        || snapshot.active_holon_space.is_none()
    {
        return Err("No acquired local Navigator session is admitted".into());
    }
    Ok(())
}

/// Native handoff only: no caller-supplied Agent/Space IDs, provisioning or constitution.
#[tauri::command]
pub async fn open_space_navigator(window: WebviewWindow) -> Result<(), String> {
    crate::trusted_welcome::admit(&window)?;
    if !cfg!(all(feature = "local-constitution", debug_assertions)) {
        return Err("Packaged Navigator is not enabled in this build".into());
    }
    let handle = window.app_handle();
    let launch = handle.state::<NavigatorLaunchLock>();
    let _guard = launch.0.try_lock().map_err(|_| "Navigator is already opening")?;
    if !handle.state::<PersonalConstitution>().available() {
        return Err("Constitution work is pending; retain it for remediation".into());
    }
    let observation = handle.state::<WelcomeLifecycle>().observation()?;
    if !matches!(observation, Some(Ok(target)) if target.sovereignty == SovereigntyObservation::AssociationPresent)
    {
        return Err("A saved sovereign association is required before opening Navigator".into());
    }
    if let Some(navigator) = handle.get_webview_window(WINDOW) {
        navigator.show().map_err(|e| e.to_string())?;
        navigator.set_focus().map_err(|e| e.to_string())?;
        window.close().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let runtime = handle
        .state::<crate::runtime::RuntimeState>()
        .read()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("MAP runtime is unavailable")?;
    prepare_application_launch(
        &runtime,
        &handle.state::<ApplicationSessionState>(),
        LaunchRequirement::AcquiredSpace,
    )
    .await
    .map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(handle, WINDOW, WebviewUrl::App("index.html".into()))
        .title("MAP — Space Navigator")
        .inner_size(1280.0, 1040.0)
        .min_inner_size(900.0, 700.0)
        .on_navigation(|url| approved_page(WINDOW, url))
        .build()
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "[HANDOFF] Opened packaged Navigator using the existing runtime, Agent and Space"
    );
    window.close().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigator_permissions_exclude_constitution_and_conductor_access() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/navigator.json")).unwrap();
        assert_eq!(capability["webviews"], serde_json::json!(["navigator"]));
        let permissions = capability["permissions"].as_array().unwrap();
        assert!(permissions.iter().any(|p| p == "allow-dispatch-map-command"));
        assert!(!permissions.iter().any(|p| p == "allow-constitute-personal-space"
            || p == "allow-map-request"
            || p.as_str().is_some_and(|name| name.starts_with("holochain:"))));
    }

    #[test]
    fn only_packaged_navigator_pages_are_operational_origins() {
        assert!(
            approved_page(WINDOW, &"tauri://localhost".parse().unwrap()),
            "Tauri simplifies index.html to the bare app origin"
        );
        for url in
            ["tauri://localhost/", "tauri://localhost/index.html", "tauri://localhost/load-holons"]
        {
            assert!(approved_page(WINDOW, &url.parse().unwrap()));
            assert!(!approved_page("welcome", &url.parse().unwrap()));
        }
        for url in [
            "http://localhost/index.html",
            "https://localhost/index.html",
            "tauri://localhost/welcome.html",
            "tauri://localhost/?bootstrap=true",
            "tauri://elsewhere/index.html",
        ] {
            assert!(!approved_page(WINDOW, &url.parse().unwrap()));
        }
    }
}
