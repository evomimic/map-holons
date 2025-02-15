
/*use dances_core::dance_request::{DanceRequest, DanceType, RequestBody};
use dances_core::dance_response::{DanceResponse, ResponseStatusCode};
use dances_core::session_state::SessionState;
use hdi::prelude::{AgentPubKey, ExternIO};
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::api::CellInfo;
use holochain::prelude::{AppBundleSource, CellId, InstalledAppId, Signal}; 
use holochain::sweettest::*;

use holons_core::core_shared_objects::{CommitRequestStatus, CommitResponse};
//use holochain_client::{
 //   AdminWebsocket, AppAuthenticationTokenIssued, AppWebsocket, AuthorizeSigningCredentialsPayload, 
 //   ClientAgentSigner, InstallAppPayload, ConductorApiError
//};
//use holochain_conductor_api::{AppInfoStatus, CellInfo, NetworkInfo};
//use holochain_types::{
  ///  app::{AppBundle, AppManifestV1, DisabledAppReason},
   // websocket::AllowedOrigins,
//};
//use holochain_zome_types::dependencies::holochain_integrity_types::ExternIO;
//use kitsune_p2p_types::{dependencies::proptest::arbitrary, fetch_pool::FetchPoolInfo};
use serde::{Deserialize, Serialize};
use shared_types_holon::{MapInteger, MapString};
use std::{io::Error, io::ErrorKind, net::Ipv4Addr};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Barrier},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestString(pub String);

pub trait ZomeClient: Sized {
 fn install(app_name:&str, happ_url:Option<&str>) -> impl std::future::Future<Output = Result<Self, ConductorApiError>> + Send;
 async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, request:DanceRequest) -> Result<CommitResponse, ConductorApiError>;
 //async fn wait_on_signal(&self, cell_id:CellId)-> Result<(), ConductorApiError>;
}

#[derive(Debug)]
pub struct AppInstallation {
    pub conductor: SweetConductor,
    pub app: SweetApp,
    pub cells: Vec<SweetCell>,
    //pub signer: ClientAgentSigner,
}

const DNA_FILEPATH: &str = "../../workdir/map_holons.dna";
const APP_ID: &str = "map_holons";
const HAPP_FILEPATH: &str = "../../workdir/map_holons.happ";


#[tokio::test(flavor = "multi_thread")]
async fn mytest() {
    let app: AppInstallation = AppInstallation::install(APP_ID,None).await.unwrap();
    //let cell_id = match app.cells[0].clone() {
            //CellInfo::Provisioned(c) => c.cell_id,
      //      _ => panic!("Invalid cell type"),
      //  };
      let cell_id = app.cells[0].cell_id().clone();
    app.zomecall(cell_id,"dances","dance", 
    DanceRequest::new(MapString("commit".to_string()), DanceType::Standalone, RequestBody::None, SessionState::empty()))
    .await.unwrap();
    //app.wait_on_signal(cell_id).await.unwrap();
}

impl ZomeClient for AppInstallation {
    async fn install(app_id:&str, happ_url:Option<&str>) -> Result<AppInstallation, ConductorApiError> {

        let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();
        let mut conductor = SweetConductor::from_standard_config().await;
        let holo_core_agent = SweetAgents::one(conductor.keystore()).await;
        let app = conductor
            .setup_app_for_agent(app_id, holo_core_agent.clone(), &[dna.clone()])
            .await
            .unwrap();

        let cells = &app.cells().clone();//[0].clone();

        //let agent_hash = holo_core_agent.into_inner();
        //let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

        Ok(Self{conductor, app, cells:cells.to_vec()})    
    }

    async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, request:DanceRequest) -> Result<DanceResponse, ConductorApiError> {

        let zome = self.conductor.get_sweet_cell(cell_id)?.zome(zome_name);//.get_cell_info(cell_id.clone()).await;
        println!("{:?}", zome);
        let response: DanceResponse  = self.conductor.call::<DanceRequest,DanceResponse>(&zome, fn_name, request).await;
        let _status = match response.status_code {
            ResponseStatusCode::OK => Ok(response),
            ResponseStatusCode::Accepted => Ok(response),
            _ => Err(ConductorApiError::ConductorError),
        };
        _status
    }

   /* async fn wait_on_signal(&self, cell_id:CellId) -> Result<(), ConductorApiError> {

        // Connect admin client
        let admin_port = self.conductor.get_arbitrary_admin_websocket_port().unwrap();
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
            //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        let credentials = admin_ws
            .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                cell_id: cell_id.clone(),
                functions: None,
            })
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
            //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;
        self.signer.add_credentials(cell_id.clone(), credentials);


        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();
        // Connect app agent client
        let app_ws_port = admin_ws
          .attach_app_interface(0, AllowedOrigins::Any, None)
          .await?;

        let token_issued = admin_ws
          .issue_app_auth_token(self.app_id.clone().into())
          .await?;
        let app_ws = AppWebsocket::connect(
            (Ipv4Addr::LOCALHOST, app_ws_port),
            token_issued.token,
            self.signer.clone().into(),
        )
        .await
        .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
        //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        app_ws
            .on_signal(move |signal| match signal {
                Signal::App { signal, .. } => {
                    let ts: TestString = signal.into_inner().decode().unwrap();
                    assert_eq!(ts.0.as_str(), "i am a signal");
                    barrier_clone.wait();
                }
                _ => panic!("Invalid signal"),
            })
            .await
            .unwrap(); 
        barrier.wait();
            Ok(())
    }*/
}

/// MOCK CONDUCTOR

pub async fn setup_conductor() -> (SweetConductor, AgentPubKey, SweetCell) {
    let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();

    // let dna_path = std::env::current_dir().unwrap().join(DNA_FILEPATH);
    // println!("{}", dna_path.to_string_lossy());
    // let dna = SweetDnaFile::from_bundle(&dna_path).await.unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;

    let holo_core_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", holo_core_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();

    let agent_hash = holo_core_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

    (conductor, agent, cell)
}*/

