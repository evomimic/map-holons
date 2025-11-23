use std::sync::{Arc, Mutex};

use holochain_types::prelude::{FunctionName, ZomeName};

use holons_core::{
    HolonsContextBehavior,
    dances::{DanceInitiator, DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode},
};
use async_trait::async_trait;
use holochain_client::{AdminWebsocket, AgentPubKey, AppWebsocket, ExternIO, ZomeCallTarget};
use base_types::MapString;
use core_types::HolonError;
use holons_client::shared_types::holon_space::SpaceInfo;
use crate::conductor_dance_caller::ConductorDanceCaller;

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
    pub async fn get_all_spaces(&self) -> Result<SpaceInfo, HolonError> {
        // TODO: needs implementation
        Ok(SpaceInfo::default())
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
        let result = app_ws.call_zome(
            ZomeCallTarget::RoleName(self.rolename.clone()),
            ZomeName::from(self.zomename.clone()),
            FunctionName::from(self.zomefunction.clone()),
            payload,
        ).await;

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