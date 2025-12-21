
use holons_client::shared_types::holon_space::{HolonSpace};
use holons_receptor::ReceptorFactory;
use tauri::{command, State};


#[command]
pub(crate) async fn load_holons (
    holon_paths: Vec<String>,
    space: HolonSpace,
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<(), String> {
    
    tracing::debug!("[TAURI COMMAND] 'load_holons' command invoked");

    receptor_factory.load_holons(space.id, holon_paths)
        .await
        .map_err(|e| format!("receptor service error: {:?}", e))?;
    Ok(())
}