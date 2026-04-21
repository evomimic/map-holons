use client_shared_types::holon_space::SpaceInfo;
use holons_client::receptor_factory::ReceptorFactory;
use tauri::{command, State};


#[command]
pub async fn root_space (
    _receptor_factory: State<'_, ReceptorFactory>,
) -> Result<SpaceInfo, String> {
    tracing::warn!("[TAURI COMMAND] 'root_space' invoked but not implemented; returning typed error");
    Err("root_space command is not implemented. Use dispatch_map_command instead or see issue #441 for follow-up.".to_string())
}
    //tracing::debug!("[TAURI COMMAND] 'root_space' command invoked");
    //let spaces = receptor_factory.all_spaces_by_type(&ReceptorType::Local)
     //   .await
      //  .map_err(|e| format!("receptor service error: {:?}", e))?;
    //Ok(spaces)

