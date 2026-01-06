
use holons_client::shared_types::holon_space::SpaceInfo;
use holons_receptor::ReceptorFactory;
use tauri::{command, State};


#[command]
pub async fn root_space (
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<SpaceInfo, String> {

    tracing::debug!("[TAURI COMMAND] 'root_space' command invoked");
    let spaces = receptor_factory.all_spaces_by_type("local")
        .await
        .map_err(|e| format!("receptor service error: {:?}", e))?;
    Ok(spaces)
}