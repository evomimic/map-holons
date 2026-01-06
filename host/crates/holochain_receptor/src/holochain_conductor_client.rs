use std::sync::{Arc, Mutex};

use holochain_types::prelude::{FunctionName, ZomeName};
use serde_bytes::ByteBuf;

use crate::conductor_dance_caller::ConductorDanceCaller;
use async_trait::async_trait;
use base_types::MapString;
use core_types::HolonError;
use holochain_client::{AdminWebsocket, AgentPubKey, AppInfo, AppWebsocket, CellInfo, ExternIO, SerializedBytes, ZomeCallTarget};
use holons_client::shared_types::holon_space::{HolonSpace, SpaceInfo};
use holons_core::{
    dances::{DanceInitiator, DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode},
    HolonsContextBehavior,
};

/// Minimal conductor client for POC.
/// Most functionality is stubbed or simplified.
#[derive(Debug, Clone)]
pub struct HolochainConductorClient {
    pub app_ws: Arc<Mutex<Option<AppWebsocket>>>,
    pub admin_ws: Arc<Mutex<Option<AdminWebsocket>>>,
    pub rolename: String,
    pub zomename: String,
    pub zomefunction: String,
    pub agent: AgentPubKey,
}

impl HolochainConductorClient {
    // NOTE: I have had to put this back to make the UI work - needs to be refactored properly later
    pub async fn get_all_spaces(&self) -> Result<SpaceInfo, HolonError> {
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
impl DanceInitiator for HolochainConductorClient {
    async fn initiate_dance(
        &self,
        _context: &(dyn HolonsContextBehavior + Send + Sync),
        request: DanceRequest,
    ) -> DanceResponse {
        self.conductor_dance_call(request).await
    }
}

/// Round-trip the zome call
#[async_trait]
impl ConductorDanceCaller for HolochainConductorClient {
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse {
        // --- Serialize request ---
        let payload: ExternIO = match ExternIO::encode(request) {
            Ok(p) => p,
            Err(e) => return server_error_response(format!("Encoding error: {:?}", e)),
        };

        // --- Clone websocket (POC safe) ---
        let ws = {
            let guard = self.app_ws.lock().unwrap();
            guard.as_ref().cloned()
        };

        let Some(app_ws) = ws else {
            return server_error_response("AppSocket not initialized".into());
        };

        // --- Make zome call ---
        let result = app_ws
            .call_zome(
                ZomeCallTarget::RoleName(self.rolename.clone()),
                ZomeName::from(self.zomename.clone()),
                FunctionName::from(self.zomefunction.clone()),
                payload,
            )
            .await;

        let Ok(extern_io) = result else {
            return server_error_response("Zome call failed".into());
        };

        match ExternIO::decode::<DanceResponse>(&extern_io) {
            Ok(decoded) => decoded,
            Err(e) => server_error_response(format!("Failed to decode dance response: {:?}", e)),
        }
    }
}

/// Minimal helper for consistent error formatting.
fn server_error_response(msg: String) -> DanceResponse {
    DanceResponse {
        status_code: ResponseStatusCode::ServerError,
        description: MapString(msg),
        body: ResponseBody::None,
        descriptor: None,
        state: None,
    }
}


// NOTE: I have had to put this back to make the UI work - needs to be refactored properly later
pub fn convert_to_space_info(app_info: AppInfo) -> Result<SpaceInfo, HolonError> {
        let mut space_info = SpaceInfo::new();
        
        for (role, cells) in app_info.cell_info.iter() {
            for cell_info in cells {
                match cell_info {
                    CellInfo::Provisioned(provisioned_cell) => {
                        let sprops = HolonSpace {
                            id: provisioned_cell.cell_id.dna_hash().to_string(),
                            name: provisioned_cell.name.clone(),
                            branch_id: Some(provisioned_cell.cell_id.dna_hash().to_string()),
                            receptor_id: "holochain".to_string(),
                            space_type: role.to_string(),
                            description:  "holochain_cell".to_string(), // Adjust as necessary
                            descriptor_id: None,
                            origin_holon_id: provisioned_cell.cell_id.dna_hash().to_string(), // Adjust if you have a way to derive this
                            metadata: Some(to_bytebuf(provisioned_cell.dna_modifiers.properties.clone())),
                            enabled: true
                        };
                        space_info.add_space(role.clone(), sprops);
                    },
                    CellInfo::Cloned(cloned_cell) => {
                        let sprops = HolonSpace {
                            id: cloned_cell.cell_id.dna_hash().to_string(),
                            name: cloned_cell.name.clone(),
                            branch_id: None,
                            receptor_id: "holochain".to_string(),
                            space_type: role.to_string(), // Adjust as necessary
                            description:  "holochain_cloned_cell".to_string(), // Adjust as necessary
                            descriptor_id: None,
                            origin_holon_id: cloned_cell.cell_id.dna_hash().to_string(), // Adjust if you have a way to derive this
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
