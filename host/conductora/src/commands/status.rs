use holons_receptor::ReceptorFactory;
use tauri::{command, State};

#[command]
pub(crate) async fn is_service_ready(
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<bool, String> {
    tracing::debug!("[TAURI COMMAND] 'is_service_ready' command invoked");
    
    // currently this waits for the holochain receptor to be loaded
    let is_ready = receptor_factory.are_receptors_loaded();

    tracing::debug!("Service ready status: {}", is_ready);

    Ok(is_ready)
}