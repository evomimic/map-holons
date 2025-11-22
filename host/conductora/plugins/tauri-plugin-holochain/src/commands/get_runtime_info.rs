use tauri::{command, AppHandle, Manager, Runtime};

use crate::HolochainPlugin;

#[command]
pub(crate) fn is_holochain_ready<R: Runtime>(app_handle: AppHandle<R>) -> bool {
    tracing::warn!("[PLUGIN COMMAND] Attempting to execute 'is_holochain_ready'");
    let is_ready = app_handle.try_state::<HolochainPlugin<R>>().is_some();
    tracing::warn!("[PLUGIN COMMAND] 'is_holochain_ready' result: {}", is_ready);
    is_ready
}
