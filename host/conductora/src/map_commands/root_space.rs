use client_shared_types::holon_space::SpaceInfo;
use holons_client::receptor_factory::ReceptorFactory;
use tauri::{command, State};


#[command]
pub async fn root_space (
    _receptor_factory: State<'_, ReceptorFactory>,
) -> Result<SpaceInfo, String> {
    unimplemented!("This command is currently a placeholder and needs to be implemented to fetch the root space information from the appropriate receptor.");

    //tracing::debug!("[TAURI COMMAND] 'root_space' command invoked");
    //let spaces = receptor_factory.all_spaces_by_type(&ReceptorType::Local)
     //   .await
      //  .map_err(|e| format!("receptor service error: {:?}", e))?;
    //Ok(spaces)
}
