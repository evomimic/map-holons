use crate::runtime::RuntimeState;
use holons_client::receptor_factory::ReceptorFactory;
use tauri::{command, State};

#[command]
pub async fn is_service_ready(
    receptor_factory: State<'_, ReceptorFactory>,
    runtime_state: State<'_, RuntimeState>,
) -> Result<bool, String> {
    tracing::debug!("[TAURI COMMAND] 'is_service_ready' command invoked");

    let receptors_loaded = receptor_factory.are_receptors_loaded();
    let runtime_ready = runtime_state.read().map(|guard| guard.is_some()).unwrap_or(false);

    let is_ready = receptors_loaded && runtime_ready;
    tracing::debug!(
        "Service ready: {} (receptors={}, runtime={})",
        is_ready,
        receptors_loaded,
        runtime_ready,
    );

    Ok(is_ready)
}
