use crate::runtime::RuntimeState;
use crate::setup::core_schema_bootstrap::CoreSchemaBootstrapGate;
use holons_client::deprecated_receptor_factory::DeprecatedReceptorFactory;
use tauri::{command, State};

#[command]
pub async fn is_service_ready(
    receptor_factory: State<'_, DeprecatedReceptorFactory>,
    runtime_state: State<'_, RuntimeState>,
    bootstrap_gate: State<'_, CoreSchemaBootstrapGate>,
) -> Result<bool, String> {
    tracing::debug!("[TAURI COMMAND] 'is_service_ready' command invoked");

    let receptors_loaded = receptor_factory.are_receptors_loaded();
    let runtime_ready = runtime_state.read().map(|guard| guard.is_some()).unwrap_or(false);

    let bootstrap_ready = bootstrap_gate.ensure_ready().is_ok();
    let is_ready = receptors_loaded && runtime_ready && bootstrap_ready;
    tracing::debug!(
        "Service ready: {} (receptors={}, runtime={}, bootstrap={})",
        is_ready,
        receptors_loaded,
        runtime_ready,
        bootstrap_ready,
    );

    Ok(is_ready)
}
