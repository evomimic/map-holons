use crate::runtime::RuntimeState;
use tauri::{command, State};

#[command]
pub async fn is_service_ready(
    runtime_state: State<'_, RuntimeState>,
) -> Result<bool, String> {
    tracing::debug!("[TAURI COMMAND] 'is_service_ready' command invoked");

    let is_ready = runtime_state
        .read()
        .map(|guard| guard.is_some())
        .unwrap_or(false);

    tracing::debug!("Service ready: {}", is_ready);

    Ok(is_ready)
}
