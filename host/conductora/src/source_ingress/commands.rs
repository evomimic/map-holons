use super::{discover_sources, pick_sources, SourceSelection, SourceSelectionMode};

/// Open the native picker, then read only the paths returned by that picker.
/// No arbitrary filesystem path is accepted from the webview.
#[tauri::command]
pub async fn select_loader_sources(
    app: tauri::AppHandle,
    mode: SourceSelectionMode,
) -> Result<SourceSelection, String> {
    #[cfg(target_os = "macos")]
    let selection = {
        let (sender, receiver) = futures::channel::oneshot::channel();
        app.run_on_main_thread(move || {
            let _ = sender.send(pick_sources(mode));
        })
        .map_err(|error| error.to_string())?;
        receiver.await.map_err(|error| error.to_string())??
    };
    #[cfg(not(target_os = "macos"))]
    let selection = {
        let _ = app;
        tauri::async_runtime::spawn_blocking(move || pick_sources(mode))
            .await
            .map_err(|error| error.to_string())??
    };
    tauri::async_runtime::spawn_blocking(move || match selection {
        None => SourceSelection::Cancelled,
        Some(paths) => SourceSelection::Selected(discover_sources(paths)),
    })
    .await
    .map_err(|error| error.to_string())
}
