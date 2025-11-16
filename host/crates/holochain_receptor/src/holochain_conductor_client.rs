use std::sync::{Arc, Mutex};
use core_types::HolonError;
use holochain_types::prelude::{FunctionName, ZomeName};
use holons_client::{shared_types::holon_space::{HolonSpace, SpaceInfo}};
use holons_core::{HolonsContextBehavior, dances::{ConductorDanceCaller, DanceInitiator, DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode}};
use async_trait::async_trait;
use holochain_client::{AdminWebsocket, AgentPubKey, AppInfo, AppWebsocket, CellInfo, ExternIO, SerializedBytes, ZomeCallTarget};
use base_types::{ MapString};
use serde_bytes::ByteBuf;

#[async_trait]
impl DanceInitiator for HolochainConductorClient {
    async fn initiate_dance(
        &self,
        _context: &dyn HolonsContextBehavior,
        request: DanceRequest,
    ) -> DanceResponse {
        self.conductor_dance_call(request).await //initiate_dance(context, request).await //dance_call(context, request).await
    }
}

#[derive(Debug, Clone)]
pub struct HolochainConductorClient {
    pub app_ws: Arc<Mutex<Option<AppWebsocket>>>,
    pub admin_ws: Arc<Mutex<Option<AdminWebsocket>>>,
    pub rolename: String,
    pub zomename: String,
    pub zomefunction: String,
    pub agent: AgentPubKey,
    //pub cell_id: CellId,
}

impl HolochainConductorClient {
    /// function uses app_info from holochain to get cells and convert to SpaceInfo
    /// this is not the whole picture as we need the holon that represent the space too
    pub async fn get_all_spaces(
        &self,
    ) -> Result<SpaceInfo, HolonError> {
        let app_websocket_clone: AppWebsocket = {
            let app_ws_guard = self.app_ws.lock().unwrap();
            app_ws_guard.as_ref()
                .ok_or_else(|| HolonError::FailedToBorrow("Service is not yet initialized with AppSocket.".into()))?
                .clone()
        }; // MutexGuard is dropped here as it goes out of scope
        
        // Now, use the cloned websocket to get the AppInfo.
        let app_info_response = app_websocket_clone.app_info().await;

        match app_info_response {
            Ok(Some(app_info)) => {
                // Successfully retrieved AppInfo, now convert it.
                let space_info = convert_to_space_info(app_info)?;
                tracing::info!("[ReceptorService] Successfully retrieved space info.");
                Ok(space_info)
            }
            Ok(None) => {
                // The zome call succeeded but returned no AppInfo.
                tracing::error!("[ReceptorService] AppInfo not found for this app.");
                Err(HolonError::NotImplemented("AppInfo not found for this app.".into()))
            }
            Err(e) => {
                // The zome call itself failed.
                tracing::error!("[ReceptorService] Error getting AppInfo: {:?}", e);
                Err(HolonError::NotImplemented(format!("Failed to get AppInfo from conductor: {:?}", e)))
            }
        }
    }
}


#[async_trait]
impl ConductorDanceCaller for HolochainConductorClient {
     async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse {
        // Serialize the dance_request into ExternIO
        let extern_io_payload: ExternIO = match ExternIO::encode(request) {
            Ok(payload) => payload,
            Err(e) => {
                tracing::error!("Failed to encode dance_request: {:?}", e);
                return error_response(format!("Encoding error: {:?}", e));
            }
        };
        // Clone the app_websocket
        let app_websocket_clone: AppWebsocket = match {
            let app_ws_guard = self.app_ws.lock().unwrap();
            app_ws_guard.as_ref().cloned()
        } {
            Some(ws) => ws,
            None => {
                tracing::error!("DanceService is not yet initialized with AppSocket.");
                return error_response("AppSocket not initialized".to_string());
            }
        };

        // Make the zome call
        let zome_response = app_websocket_clone.call_zome(
            ZomeCallTarget::RoleName(self.rolename.clone()),
            ZomeName::from(self.zomename.clone()),
            FunctionName::from(self.zomefunction.clone()),
            extern_io_payload,
        ).await;

        // Handle the response
        match zome_response {
            Ok(extern_io) => {
                match ExternIO::decode::<DanceResponse>(&extern_io) {
                    Ok(dance_response) => {
                        tracing::info!("[CONDUCTOR DANCE CALL] Zome call response: {:?}", dance_response);
                        dance_response
                    }
                    Err(e) => {
                        tracing::error!("Failed to decode dance_response: {:?}", e);
                        error_response(format!("Failed to decode dance_response: {:?}", e))
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to call zome: {:?}", e);
                error_response(format!("Zome call error: {:?}", e))
            }
        }
    }
}
    /// Helper function to create error DanceResponse
    fn error_response(description: String) -> DanceResponse {
        DanceResponse {
            status_code: ResponseStatusCode::ServerError,
            description: MapString(description),
            body: ResponseBody::None,
            descriptor: None,
            state: None,
        }
    }



pub fn convert_to_space_info(app_info: AppInfo) -> Result<SpaceInfo, HolonError> {
        let mut space_info = SpaceInfo::new();
        
        // Assuming app_info has a method to get spaces or similar data
        // This is a placeholder; adjust according to actual AppInfo structure
        for (role, cells) in app_info.cell_info.iter() {
            for cell_info in cells {
                // You need to match on the cell_info enum variant, e.g. Provisioned, Cloned, etc.
                // Adjust the match arms according to the actual enum and struct definitions in your codebase.
                match cell_info {
                    CellInfo::Provisioned(provisioned_cell) => {
                        let sprops = HolonSpace {
                            id: provisioned_cell.cell_id.dna_hash().to_string(),
                            receptor_id: "holochain".to_string(),
                            branch_id: None,
                            name: provisioned_cell.name.clone(),
                            space_type: role.to_string(),
                            description:  "holochain_cell".to_string(), // Adjust as necessary
                            descriptor_id: None,
                            origin_space_id: provisioned_cell.cell_id.dna_hash().to_string(), // Adjust if you have a way to derive this
                            metadata: Some(to_bytebuf(provisioned_cell.dna_modifiers.properties.clone())),
                            enabled: true
                        };
                        space_info.add_space(role.clone(), sprops);
                    },
                    CellInfo::Cloned(cloned_cell) => {
                        let sprops = HolonSpace {
                            id: cloned_cell.cell_id.dna_hash().to_string(),
                            receptor_id: "holochain".to_string(),
                            branch_id: None,
                            name: cloned_cell.name.clone(),
                            space_type: role.to_string(), // Adjust as necessary
                            description:  "holochain_cloned_cell".to_string(), // Adjust as necessary
                            descriptor_id: None,
                            origin_space_id: cloned_cell.cell_id.dna_hash().to_string(), // Adjust if you have a way to derive this
                            metadata: Some(to_bytebuf(cloned_cell.dna_modifiers.properties.clone())),
                            enabled: cloned_cell.enabled,
                        };
                        space_info.add_space(role.clone(), sprops);},
                    _ => {
                        // Handle other cell types if necessary
                    }
                }
            }
        }
        Ok(space_info)
    }

    fn to_bytebuf(sb: SerializedBytes) -> ByteBuf {
        // SerializedBytes implements `.bytes()` which gives you a reference to its Vec<u8>
        let v: Vec<u8> = sb.bytes().to_vec();
        ByteBuf::from(v)
}