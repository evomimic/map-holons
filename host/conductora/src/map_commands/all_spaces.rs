
use holons_client::shared_types::holon_space::SpaceInfo;
use holons_receptor::ReceptorFactory;
use tauri::{command, State};


#[command]
pub async fn all_spaces (
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<SpaceInfo, String> {

    tracing::debug!("[TAURI COMMAND] 'all_spaces' command invoked");

    let spaces = receptor_factory.all_spaces_by_type("holochain")
        .await
        .map_err(|e| format!("receptor service error: {:?}", e))?;
    Ok(spaces)
}