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
) -> Result<MapIpcResponse, ()> {
    tracing::debug!("[TAURI COMMAND] 'dispatch_map_command' invoked");

    let request_id = request.request_id;

    let result = dispatch_inner(&request_id, request.command, request.options, runtime_state).await;

    let wire_result = match result {
        Ok(domain_result) => Ok(MapResultWire::from(domain_result)),
        Err(error) => Err(error),
    };

    // Always Ok — all domain errors are inside the envelope.
    Ok(MapIpcResponse { request_id, result: wire_result })
}

/// Inner dispatch that returns `Result` so early errors are captured in the
/// response envelope rather than escaping as a bare Tauri error.
async fn dispatch_inner(
    request_id: &map_commands_wire::RequestId,
    command: MapCommandWire,
    options: map_commands_wire::RequestOptions,
    runtime_state: State<'_, RuntimeState>,
) -> Result<map_commands_contract::MapResult, HolonError> {
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

    // Log gesture context if present
    if let Some(ref gesture_id) = options.gesture_id {
        let label = options.gesture_label.as_deref().unwrap_or("<no label>");
        tracing::info!(
            "dispatch_map_command request_id={} gesture_id={:?} label={}",
            request_id.value(),
            gesture_id.0,
            label
        );
    }

    // Bind wire → domain
    let command = bind_command(&runtime, command)?;

    // Execute via runtime (policy enforcement + handler routing)
    runtime.execute_command(command).await
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
