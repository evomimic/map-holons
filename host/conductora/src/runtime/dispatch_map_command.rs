use std::sync::RwLock;

use core_types::HolonError;
use map_commands_contract::MapCommand;
use map_commands_runtime::Runtime;
use map_commands_wire::{
    MapCommandWire, MapIpcRequest, MapIpcResponse, MapResultWire,
};
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

    let request_id = request.request_id;

    // Log gesture context if present
    if let Some(ref gesture_id) = request.options.gesture_id {
        let label = request.options.gesture_label.as_deref().unwrap_or("<no label>");
        tracing::info!(
            "dispatch_map_command request_id={} gesture_id={:?} label={}",
            request_id.value(),
            gesture_id.0,
            label
        );
    }

    // Bind wire → domain
    let command = bind_command(&runtime, request.command)?;

    // Execute via runtime (policy enforcement + handler routing)
    let result = runtime.execute_command(command).await;

    // Convert domain result → wire
    let wire_result = match result {
        Ok(domain_result) => Ok(MapResultWire::from(domain_result)),
        Err(error) => Err(error),
    };

    Ok(MapIpcResponse { request_id, result: wire_result })
}

/// Binds a wire command to its domain equivalent using the runtime session.
fn bind_command(
    runtime: &Runtime,
    command: MapCommandWire,
) -> Result<MapCommand, HolonError> {
    match command {
        MapCommandWire::Space(wire) => Ok(MapCommand::Space(wire.bind())),
        MapCommandWire::Transaction(wire) => {
            let context = runtime.session().get_transaction(&wire.tx_id)?;
            Ok(MapCommand::Transaction(wire.bind(context)?))
        }
        MapCommandWire::Holon(wire) => {
            let context = runtime.session().get_transaction(&wire.tx_id)?;
            Ok(MapCommand::Holon(wire.bind(&context)?))
        }
    }
}
