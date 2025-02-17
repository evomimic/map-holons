// use holochain_zome_types::prelude::{CellId, ZomeCallResponse};
// use holons_core::core_shared_objects::HolonError;
// use std::net::Ipv4Addr;
// use std::sync::Arc;
//
// use holochain_client::{AppAuthenticationToken, AppWebsocket, ClientAgentSigner, ZomeCallTarget};
// use holochain_types::prelude::*;
// use serde_json::json;
// use std::time::Duration;
//
// /// Stores configuration needed to interact with a Holochain conductor.
// pub struct ConductorConfig {
//     pub app_api_port: u16,
//     pub app_auth_token: Option<AppAuthenticationToken>, // Optional for Sweettest
//     pub agent_signer: Option<Arc<ClientAgentSigner>>,   // Optional for Sweettest
// }
//
// impl ConductorConfig {
//     /// Creates a new ConductorConfig for a real conductor (production)
//     pub fn new(
//         app_api_port: u16,
//         app_auth_token: AppAuthenticationToken,
//         agent_signer: Arc<ClientAgentSigner>,
//     ) -> Self {
//         Self {
//             app_api_port,
//             app_auth_token: Some(app_auth_token),
//             agent_signer: Some(agent_signer),
//         }
//     }
//
//     /// Creates a new ConductorConfig for a mock conductor (Sweettest)
//     pub fn mock(app_api_port: u16) -> Self {
//         Self {
//             app_api_port,
//             app_auth_token: None, // No auth needed in Sweettest
//             agent_signer: None,   // No signing needed in Sweettest
//         }
//     }
// }
// pub struct ZomeClient;
//
// impl ZomeClient {
//     pub async fn zomecall(
//         &self,
//         conductor_config: &ConductorConfig,
//         cell_id: CellId,
//         zome_name: &str,
//         fn_name: &str,
//         request: DanceRequest,
//     ) -> Result<DanceResponse, HolonError> {
//         let app_ws_url = (Ipv4Addr::LOCALHOST, conductor_config.app_api_port);
//
//         // ✅ Ensure we always have an auth token and signer (for Sweettest too)
//         let auth_token = conductor_config.app_auth_token.clone().unwrap_or_else(|| {
//             AppAuthenticationToken::new_random() // Generates a dummy token
//         });
//
//         let agent_signer = conductor_config.agent_signer.clone().unwrap_or_else(|| {
//             Arc::new(ClientAgentSigner::default()) // Creates a default signer
//         });
//
//         // ✅ Connect to the App WebSocket (always passing three arguments)
//         let app_ws = AppWebsocket::connect(app_ws_url, auth_token, agent_signer)
//             .await
//             .map_err(|e| HolonError::Misc(format!("App WebSocket connection failed: {}", e)))?;
//
//         let target = ZomeCallTarget::CellId(cell_id.clone());
//
//         let payload = ExternIO::encode(request)
//             .map_err(|e| HolonError::Misc(format!("Failed to serialize request: {}", e)))?;
//
//         let response = app_ws
//             .call_zome(
//                 target,
//                 ZomeName::from(zome_name.to_string()),
//                 FunctionName::from(fn_name.to_string()),
//                 payload,
//             )
//             .await
//             .map_err(|e| HolonError::Misc(format!("Zome call failed: {}", e)))?;
//
//         response
//             .decode::<DanceResponse>()
//             .map_err(|e| HolonError::Misc(format!("Failed to deserialize response: {}", e)))
//     }
// }
