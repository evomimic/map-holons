use std::sync::RwLock;

use core_types::HolonError;
use map_commands::dispatch::Runtime;
use map_commands::wire::{MapIpcRequest, MapIpcResponse};
use tauri::{command, State};

/// Tauri-managed state wrapper for the MAP Commands runtime.
///
/// Initially `None` until Holochain setup completes and the Runtime
/// is constructed in `run_complete_setup`.
pub type RuntimeState = RwLock<Option<Runtime>>;

#[command]
pub async fn dispatch_map_command(
    request: MapIpcRequest,
    runtime_state: State<'_, RuntimeState>,
) -> Result<MapIpcResponse, HolonError> {
    tracing::debug!("[TAURI COMMAND] 'dispatch_map_command' invoked");

    // Clone the Runtime out of the lock so we don't hold it across await.
    let runtime = runtime_state
        .read()
        .map_err(|e| {
            HolonError::FailedToAcquireLock(format!("RuntimeState lock poisoned: {}", e))
        })?
        .clone();

    let runtime = runtime.ok_or_else(|| {
        HolonError::ServiceNotAvailable(
            "MAP Commands Runtime not initialized".to_string(),
        )
    })?;

    runtime.dispatch(request).await
}
