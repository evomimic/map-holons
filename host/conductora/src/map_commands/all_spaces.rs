
use client_shared_types::{base_receptor::ReceptorType, holon_space::SpaceInfo};
use holons_client::receptor_factory::{ReceptorFactory};
use tauri::{command, State};


#[command]
pub async fn all_spaces (
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<SpaceInfo, String> {

    tracing::debug!("[TAURI COMMAND] 'all_spaces' command invoked");

    let spaces = receptor_factory.all_spaces_by_type(&ReceptorType::Holochain).await
        .map_err(|e| format!("receptor service error: {:?}", e))?;
    Ok(spaces)
}